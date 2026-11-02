use std::collections::HashMap;

use es_geom::Rect;

use crate::{Line, Word};

const WORD_LEVEL: u32 = 5;
const FIELDS: usize = 12;

pub fn parse_tsv(tsv: &str) -> Vec<Word> {
    let mut line_ids: HashMap<(u32, u32, u32), u32> = HashMap::new();
    let mut words = Vec::new();
    for row in tsv.lines() {
        let f: Vec<&str> = row.splitn(FIELDS, '\t').collect();
        if f.len() < FIELDS {
            continue;
        }
        let num = |i: usize| f[i].trim().parse::<u32>().ok();
        if num(0) != Some(WORD_LEVEL) {
            continue;
        }
        let (Some(block), Some(par), Some(line)) = (num(2), num(3), num(4)) else {
            continue;
        };
        let (Some(x), Some(y), Some(w), Some(h)) = (num(6), num(7), num(8), num(9)) else {
            continue;
        };
        let Ok(confidence) = f[10].trim().parse::<f32>() else {
            continue;
        };
        let text = f[11].trim();
        if text.is_empty() {
            continue;
        }
        let next = line_ids.len() as u32;
        let line = *line_ids.entry((block, par, line)).or_insert(next);
        words.push(Word {
            text: text.to_owned(),
            rect: Rect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64),
            confidence,
            line,
        });
    }
    words
}

pub(crate) fn lines(words: &[Word]) -> Vec<Line> {
    let mut out: Vec<(u32, Line)> = Vec::new();
    for w in words {
        match out.last_mut() {
            Some((id, line)) if *id == w.line => {
                line.text.push(' ');
                line.text.push_str(&w.text);
                line.rect = line.rect.union(w.rect);
            }
            _ => out.push((
                w.line,
                Line {
                    text: w.text.clone(),
                    rect: w.rect,
                },
            )),
        }
    }
    out.into_iter().map(|(_, l)| l).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
1\t1\t0\t0\t0\t0\t0\t0\t900\t160\t-1\t\n\
4\t1\t1\t1\t1\t0\t20\t20\t300\t30\t-1\t\n\
5\t1\t1\t1\t1\t1\t20\t20\t100\t30\t91.5\tHello\n\
5\t1\t1\t1\t1\t2\t130\t22\t60\t28\t88.25\tthere\n\
5\t1\t1\t1\t1\t3\t200\t22\t10\t28\t-1\t  \n\
5\t1\t1\t1\t2\t1\t20\t80\t90\t30\t95\tabc\n\
5\t1\t2\t1\t1\t1\t20\t140\t90\t30\t80\tFar\n";

    #[test]
    fn only_non_empty_words_with_boxes_and_confidence() {
        let words = parse_tsv(SAMPLE);
        let texts: Vec<_> = words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["Hello", "there", "abc", "Far"]);
        assert_eq!(words[0].rect, Rect::new(20.0, 20.0, 120.0, 50.0));
        assert_eq!(words[1].confidence, 88.25);
    }

    #[test]
    fn line_ids_follow_block_paragraph_line() {
        let ids: Vec<_> = parse_tsv(SAMPLE).iter().map(|w| w.line).collect();
        assert_eq!(ids, [0, 0, 1, 2]);
    }

    #[test]
    fn lines_join_words_with_spaces_and_union_boxes() {
        let l = lines(&parse_tsv(SAMPLE));
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].text, "Hello there");
        assert_eq!(l[0].rect, Rect::new(20.0, 20.0, 190.0, 50.0));
    }

    #[test]
    fn garbage_yields_nothing() {
        assert!(parse_tsv("").is_empty());
        assert!(parse_tsv("not\ttsv\n5\tx").is_empty());
    }
}
