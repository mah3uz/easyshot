use crate::worker::State;
use es_geom::Rect;
use es_model::platform::{OutputEvent, OutputId, OutputInfo, Transform};
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_v1::{self, ZxdgOutputV1};

pub(crate) struct OutputState {
    pub global: u32,
    pub wl: WlOutput,
    pub xdg: Option<ZxdgOutputV1>,
    name: Option<String>,
    description: String,
    mode: (i32, i32),
    refresh_mhz: i32,
    scale: i32,
    transform: Transform,
    position: (i32, i32),
    xdg_position: Option<(i32, i32)>,
    xdg_size: Option<(i32, i32)>,
    pub announced: Option<OutputInfo>,
}

impl OutputState {
    pub fn new(global: u32, wl: WlOutput) -> Self {
        OutputState {
            global,
            wl,
            xdg: None,
            name: None,
            description: String::new(),
            mode: (0, 0),
            refresh_mhz: 0,
            scale: 1,
            transform: Transform::Normal,
            position: (0, 0),
            xdg_position: None,
            xdg_size: None,
            announced: None,
        }
    }

    pub fn attach_xdg(&mut self, manager: &ZxdgOutputManagerV1, qh: &QueueHandle<State>) {
        if self.xdg.is_none() {
            self.xdg = Some(manager.get_xdg_output(&self.wl, qh, self.global));
        }
    }

    fn info(&self) -> Option<OutputInfo> {
        let name = self.name.clone()?;
        if self.mode.0 <= 0 || self.mode.1 <= 0 || (self.xdg.is_some() && self.xdg_size.is_none()) {
            return None;
        }
        Some(output_info(
            OutputId(name),
            self.description.clone(),
            &Geometry {
                mode: self.mode,
                refresh_mhz: self.refresh_mhz,
                scale: self.scale,
                transform: self.transform,
                position: self.xdg_position.unwrap_or(self.position),
                logical_size: self.xdg_size,
            },
        ))
    }

    pub fn transform(&self) -> Transform {
        self.transform
    }
}

pub(crate) struct Geometry {
    pub mode: (i32, i32),
    pub refresh_mhz: i32,
    pub scale: i32,
    pub transform: Transform,
    pub position: (i32, i32),
    pub logical_size: Option<(i32, i32)>,
}

pub(crate) fn swaps_axes(t: Transform) -> bool {
    matches!(
        t,
        Transform::R90 | Transform::R270 | Transform::F90 | Transform::F270
    )
}

pub(crate) fn output_info(id: OutputId, description: String, g: &Geometry) -> OutputInfo {
    let (mw, mh) = (g.mode.0.max(0) as u32, g.mode.1.max(0) as u32);
    let physical = if swaps_axes(g.transform) {
        (mh, mw)
    } else {
        (mw, mh)
    };
    let int_scale = f64::from(g.scale.max(1));
    let (lw, lh) = match g.logical_size {
        Some((w, h)) if w > 0 && h > 0 => (f64::from(w), f64::from(h)),
        _ => (
            f64::from(physical.0) / int_scale,
            f64::from(physical.1) / int_scale,
        ),
    };
    // The integer wl_output scale is rounded up on fractional outputs; the true scale is pixels per logical unit.
    let scale = if lw > 0.0 {
        f64::from(physical.0) / lw
    } else {
        int_scale
    };
    let (x, y) = (f64::from(g.position.0), f64::from(g.position.1));
    OutputInfo {
        id,
        description,
        logical: Rect::new(x, y, x + lw, y + lh),
        physical,
        scale,
        transform: g.transform,
        refresh_mhz: g.refresh_mhz.max(0) as u32,
        focused: None,
    }
}

pub(crate) fn transform_from_wl(t: WEnum<wl_output::Transform>) -> Transform {
    match t {
        WEnum::Value(wl_output::Transform::_90) => Transform::R90,
        WEnum::Value(wl_output::Transform::_180) => Transform::R180,
        WEnum::Value(wl_output::Transform::_270) => Transform::R270,
        WEnum::Value(wl_output::Transform::Flipped) => Transform::Flipped,
        WEnum::Value(wl_output::Transform::Flipped90) => Transform::F90,
        WEnum::Value(wl_output::Transform::Flipped180) => Transform::F180,
        WEnum::Value(wl_output::Transform::Flipped270) => Transform::F270,
        _ => Transform::Normal,
    }
}

impl State {
    fn output_mut(&mut self, global: u32) -> Option<&mut OutputState> {
        self.outputs.iter_mut().find(|o| o.global == global)
    }

    fn announce(&mut self, global: u32) {
        let Some(o) = self.outputs.iter_mut().find(|o| o.global == global) else {
            return;
        };
        let Some(info) = o.info() else { return };
        let event = match &o.announced {
            None => OutputEvent::Added(info.clone()),
            Some(prev) if *prev != info => OutputEvent::Changed(info.clone()),
            Some(_) => return,
        };
        o.announced = Some(info);
        self.publish_outputs();
        let _ = self.output_tx.try_send(event);
    }

    pub(crate) fn remove_output(&mut self, global: u32) {
        let Some(i) = self.outputs.iter().position(|o| o.global == global) else {
            return;
        };
        let o = self.outputs.remove(i);
        if let Some(xdg) = &o.xdg {
            xdg.destroy();
        }
        if o.wl.version() >= 3 {
            o.wl.release();
        }
        if let Some(info) = o.announced {
            self.publish_outputs();
            let _ = self.output_tx.try_send(OutputEvent::Removed(info.id));
        }
    }

    pub(crate) fn publish_outputs(&self) {
        let list = self
            .outputs
            .iter()
            .filter_map(|o| o.announced.clone())
            .collect();
        *self
            .output_snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = list;
    }
}

impl Dispatch<WlOutput, u32> for State {
    fn event(
        state: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let global = *global;
        if let wl_output::Event::Done = event {
            state.announce(global);
            return;
        }
        let Some(o) = state.output_mut(global) else {
            return;
        };
        match event {
            wl_output::Event::Geometry {
                x, y, transform, ..
            } => {
                o.position = (x, y);
                o.transform = transform_from_wl(transform);
            }
            wl_output::Event::Mode {
                flags,
                width,
                height,
                refresh,
            } => {
                let current =
                    matches!(flags, WEnum::Value(f) if f.contains(wl_output::Mode::Current));
                if current {
                    o.mode = (width, height);
                    o.refresh_mhz = refresh;
                }
            }
            wl_output::Event::Scale { factor } => o.scale = factor,
            wl_output::Event::Name { name } => o.name = Some(name),
            wl_output::Event::Description { description } => o.description = description,
            _ => {}
        }
    }
}

impl Dispatch<ZxdgOutputV1, u32> for State {
    fn event(
        state: &mut Self,
        _: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let global = *global;
        if let zxdg_output_v1::Event::Done = event {
            state.announce(global);
            return;
        }
        let Some(o) = state.output_mut(global) else {
            return;
        };
        match event {
            zxdg_output_v1::Event::LogicalPosition { x, y } => o.xdg_position = Some((x, y)),
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                o.xdg_size = Some((width, height))
            }
            zxdg_output_v1::Event::Name { name } if o.wl.version() < 4 => o.name = Some(name),
            zxdg_output_v1::Event::Description { description } if o.wl.version() < 4 => {
                o.description = description
            }
            _ => {}
        }
    }
}

impl Dispatch<ZxdgOutputManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZxdgOutputManagerV1,
        _: <ZxdgOutputManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom(mode: (i32, i32), scale: i32, t: Transform, logical: Option<(i32, i32)>) -> Geometry {
        Geometry {
            mode,
            refresh_mhz: 60000,
            scale,
            transform: t,
            position: (100, 50),
            logical_size: logical,
        }
    }

    fn info(g: &Geometry) -> OutputInfo {
        output_info(OutputId("HEADLESS-1".into()), String::new(), g)
    }

    #[test]
    fn fractional_scale_comes_from_xdg_logical_size_not_the_integer_scale() {
        let i = info(&geom((1920, 1080), 2, Transform::Normal, Some((1280, 720))));
        assert_eq!(i.physical, (1920, 1080));
        assert_eq!(i.scale, 1.5);
        assert_eq!(i.logical, Rect::new(100.0, 50.0, 1380.0, 770.0));
    }

    #[test]
    fn rotated_outputs_report_the_physical_size_as_displayed() {
        let i = info(&geom((2560, 1440), 1, Transform::R90, Some((1440, 2560))));
        assert_eq!(i.physical, (1440, 2560));
        assert_eq!(i.scale, 1.0);
        let flipped = info(&geom((2560, 1440), 1, Transform::F270, None));
        assert_eq!(flipped.logical.size().width, 1440.0);
    }

    #[test]
    fn without_xdg_output_the_integer_scale_divides_the_mode() {
        let i = info(&geom((3840, 2160), 2, Transform::Normal, None));
        assert_eq!(i.scale, 2.0);
        assert_eq!(i.logical, Rect::new(100.0, 50.0, 2020.0, 1130.0));
    }

    #[test]
    fn every_wayland_transform_maps_to_its_own_variant() {
        use wl_output::Transform as W;
        let all = [
            W::Normal,
            W::_90,
            W::_180,
            W::_270,
            W::Flipped,
            W::Flipped90,
            W::Flipped180,
            W::Flipped270,
        ];
        let mapped: Vec<Transform> = all
            .iter()
            .map(|t| transform_from_wl(WEnum::Value(*t)))
            .collect();
        for (i, a) in mapped.iter().enumerate() {
            assert!(mapped[i + 1..].iter().all(|b| b != a), "{a:?} mapped twice");
        }
        assert_eq!(transform_from_wl(WEnum::Unknown(42)), Transform::Normal);
    }
}
