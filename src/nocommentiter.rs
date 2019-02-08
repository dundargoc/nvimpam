//! This module holds [`NoCommentIter`](::nocommentiter::NoCommentIter), the
//! central datastructure to parse the lines of a buffer.
//!
//! It returns enumerated Lines, but skips Comments (lines starting with `$` or
//! `#`). All skip functions, used by
//! [`parse_from_iter`](::bufdata::BufData::parse_from_iter), work on a
//! [`NoCommentIter`](::nocommentiter::NoCommentIter).
use crate::{
  bufdata::highlights::Highlights,
  card::{
    ges::GesType,
    keyword::Keyword,
    line::{CondResult, Line as CardLine},
    Card,
  },
  lines::{KeywordLine, ParsedLine},
  skipresult::SkipResult,
};

// Used in skip functions. Returns the next `ParsedLine` from the iterator. If
// theres no next line, return a `SkipResult` containing the line number of
// `prevline` and nothing else.
macro_rules! next_or_return_previdx {
  ($self:ident, $previdx:expr) => {
    match $self.next() {
      None => {
        return SkipResult {
          skip_end: $previdx,
          nextline: None,
        };
      }
      Some(t) => t,
    };
  };
}

// Used in skip_ges to get the next line. If it's None, we're at the end of
// the file and only return what we found before. Also used in `advance_some!`
macro_rules! next_or_return_some_previdx {
  ($self:ident, $previdx:expr) => {
    match $self.next() {
      None => {
        return Some(SkipResult {
          skip_end: $previdx,
          nextline: None,
        });
      }
      Some(t) => t,
    };
  };
}

// In the same veins as above, get the next line from the iterator, or return
// None from the function.
macro_rules! next_or_return_none {
  ($self:ident) => {
    match $self.next() {
      None => return None,
      Some(t) => t,
    };
  };
}

// A common pattern for nocommentiter: Save Some(nextline) in prevline,
// and advance the iterator. Save in nextline, or return a SkipResult built
// from prevline's line number
macro_rules! advance {
  ($self:ident, $previdx:ident, $nextline:ident) => {
    $previdx = $nextline.number;
    $nextline = next_or_return_previdx!($self, $previdx);
  };
}

// Same as advance above, just that the `SkipResult` is wrapped in `Some`. Used
// in skip_ges.
macro_rules! advance_some {
  ($self:ident, $previdx:ident, $nextline:ident) => {
    $previdx = $nextline.number;
    $nextline = next_or_return_some_previdx!($self, $previdx);
  };
}

/// Designates that the comments have been removed.
pub trait CommentLess {
  fn remove_comments(self) -> NoCommentIter<Self>
  where
    Self: Sized;
}

/// The struct simply holds a type instance. Skipping comments is done in the
/// Iterator implementation.
pub struct NoCommentIter<I> {
  pub it: I,
}

impl<'a, I> Iterator for NoCommentIter<I>
where
  I: Iterator<Item = ParsedLine<'a>>,
{
  type Item = ParsedLine<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    while let Some(pl) = self.it.next() {
      if pl.keyword != Some(&Keyword::Comment) {
        return Some(pl);
      }
    }
    None
  }
}

impl<'a, I> CommentLess for I
where
  I: Iterator<Item = ParsedLine<'a>>,
{
  fn remove_comments(self) -> NoCommentIter<Self> {
    NoCommentIter { it: self }
  }
}

impl<'a, I> NoCommentIter<I>
where
  I: Iterator<Item = ParsedLine<'a>>,
{
  /// Advance the iterator until meeting the first line with a keyword. If the
  /// file ends before that, return `None`.
  pub fn skip_to_next_keyword<'b>(&'b mut self) -> Option<KeywordLine<'a>> {
    let mut line = None;

    while line.is_none() {
      line = next_or_return_none!(self).try_into_keywordline();
    }

    line
  }

  /// Advance the iterator until the first line after a General Entity
  /// Selection (GES).
  ///
  /// Returns `None` if skipline neither ends the GES, nor is
  /// contained in it. We do not try to advance the iterator in this case.
  pub fn skip_ges<'b>(
    &'b mut self,
    ges: GesType,
    skipline: &ParsedLine<'a>,
  ) -> Option<SkipResult<'a>> {
    let mut previdx: i64 = skipline.number;
    let mut nextline: ParsedLine<'a>;

    let contained = ges.contains(skipline.text);
    let ends = ges.ended_by(skipline.text);

    if ends {
      nextline = next_or_return_some_previdx!(self, previdx);
      Some(SkipResult {
        nextline: Some(nextline),
        skip_end: previdx,
      })
    } else if !ends && !contained {
      None
    } else {
      nextline = next_or_return_some_previdx!(self, skipline.number);

      while ges.contains(nextline.text) {
        advance_some!(self, previdx, nextline);
      }

      if ges.ended_by(nextline.text) {
        advance_some!(self, previdx, nextline);
      }

      Some(SkipResult {
        nextline: Some(nextline),
        skip_end: previdx,
      })
    }
  }

  /// A wrapper around [`skip_card`](NoCommentIter::skip_card) and
  /// [`skip_card_gather`](NoCommentIter::skip_card_gather), dispatching by
  /// value of [`Card.ownfold`](::card::Card::ownfold)
  pub fn skip_fold<'b>(
    &'b mut self,
    skipline: &KeywordLine<'a>,
    highlights: &mut Highlights,
  ) -> SkipResult<'a> {
    let card: &Card = skipline.keyword.into();

    if card.ownfold {
      self.skip_card(&skipline, card, highlights)
    } else {
      self.skip_card_gather(&skipline, card, highlights)
    }
  }

  /// Let [`NoCommentIter`](NoCommentIter) skip the given
  /// [`Card`](::card::Card), but only skip this 1 card. This only really makes
  /// sense when the last line the iterator returned is the line with the
  /// keyword starting that card, which is passed as `skipline`.
  ///
  /// If you want to skip all cards of a given type, use
  /// [`skip_card_gather`](NoCommentIter::skip_card_gather)
  pub fn skip_card<'b>(
    &'b mut self,
    skipline: &KeywordLine<'a>,
    card: &Card,
    highlights: &mut Highlights,
  ) -> SkipResult<'a> {
    let mut conds: Vec<CondResult> = vec![]; // the vec to hold the conditionals
    let mut cardlines = card.lines.iter();
    let cardline = cardlines.next().unwrap_or_else(|| unreachable!());

    if let CardLine::Provides(_s, ref c) = cardline {
      conds.push(c.evaluate(skipline.text));
    }

    highlights
      .add_line_highlights(skipline.number, cardline.highlights(skipline.text));

    let mut previdx: i64 = skipline.number;
    let mut nextline = next_or_return_previdx!(self, previdx);

    for cardline in cardlines {
      if nextline.keyword.is_some() {
        break;
      }

      match *cardline {
        CardLine::Provides(_s, ref c) => {
          conds.push(c.evaluate(&nextline.text));
          advance!(self, previdx, nextline);
        }
        CardLine::Ges(ref g) => {
          if let Some(sr) = self.skip_ges(*g, &nextline) {
            match sr.nextline {
              None => return sr,
              Some(pl) => {
                previdx = sr.skip_end;
                nextline = pl;
              }
            };
          }
        }
        CardLine::Cells(_s) => {
          highlights.add_line_highlights(
            nextline.number,
            cardline.highlights(nextline.text),
          );
          advance!(self, previdx, nextline);
        }
        CardLine::Optional(_s, i) => {
          if conds.get(i as usize) == Some(&CondResult::Bool(true)) {
            advance!(self, previdx, nextline);
          } else {
            continue;
          }
        }
        CardLine::Repeat(_s, i) => {
          let num = match conds.get(i as usize) {
            Some(CondResult::Number(Some(u))) if *u > 0 => u,
            _ => continue,
          };

          // TODO(KillTheMule): Is this comment still right? Guess not...
          // We need one more loop than *num because we need to get the next
          // line for the next outer iteration
          for _ in 0..*num {
            advance!(self, previdx, nextline);

            if nextline.keyword.is_some() {
              break;
            }
          }
        }
        CardLine::Block(_l, s) => loop {
          while !nextline.text.starts_with(s) {
            advance!(self, previdx, nextline);

            if nextline.keyword.is_some() {
              break;
            }
          }
          advance!(self, previdx, nextline);
        },
        CardLine::OptionalBlock(s1, s2) => {
          if !nextline.text.starts_with(s1) {
            continue;
          }
          while !nextline.text.starts_with(s2) {
            advance!(self, previdx, nextline);

            if nextline.keyword.is_some() {
              break;
            }
          }
        }
      }
    }
    SkipResult {
      nextline: Some(nextline),
      skip_end: previdx,
    }
  }

  /// Let [`NoCommentIter`](NoCommentIter) skip all given
  /// [`Card`](::card::Card)s, until the next different card starts. The basic
  /// assumption is that the last line the iterator returned is a the first line
  /// of a card of the given type, which is passed as `skipline`.
  pub fn skip_card_gather<'b>(
    &'b mut self,
    skipline: &KeywordLine<'a>,
    card: &Card,
    hls: &mut Highlights,
  ) -> SkipResult<'a> {
    let mut res = self.skip_card(&skipline, card, hls);

    #[cfg_attr(rustfmt, rustfmt_skip)]
    while let Some(ParsedLine{keyword: Some(k), number, text}) = res.nextline {
      if *k == card.keyword() {
        res = self.skip_card(&KeywordLine{keyword: k, number, text}, card, hls);
      } else {
        break
      }
    }

    res
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    bufdata::BufData,
    card::{
      ges::GesType::GesNode,
      keyword::{
        Keyword::{self, *},
        Keywords,
      },
    },
    carddata::*,
    lines::{KeywordLine, Lines, ParsedLine},
    nocommentiter::{CommentLess, NoCommentIter},
  };

  macro_rules! pline {
    ($number:expr, $text:expr, $keyword:expr) => {
      ParsedLine {
        number: $number,
        text: $text.as_ref(),
        keyword: $keyword,
      }
    };
  }

  macro_rules! kwline {
    ($number:expr, $text:expr, $keyword:expr) => {
      KeywordLine {
        number: $number,
        text: $text,
        keyword: $keyword,
      }
    };
  }

  macro_rules! make_lineiter {
    ($lines:ident, $keywords:ident, $li: ident, $str:expr) => {
      $lines.parse_slice($str.as_ref());
      $keywords.parse_lines(&$lines);
      $li = (0i64..)
        .zip($keywords.iter().zip($lines.iter()))
        .map(ParsedLine::from)
        .remove_comments()
    };
  }

  macro_rules! make_test {
    ($name: ident, $strs: expr, $({$f:expr, $e:expr});+) => {
      #[test]
      fn $name() {
        let mut lines = Lines::new();
        let mut keywords = Keywords::new();
        let mut li;
        make_lineiter!(lines, keywords, li, $strs);
        $( assert_eq!($f(&mut li), $e) );+
      }
    };
  }

  fn next_kw<'a, I>(l: &mut NoCommentIter<I>) -> Option<KeywordLine<'a>>
  where
    I: Iterator<Item = ParsedLine<'a>>,
  {
    l.skip_to_next_keyword()
  }

  fn next_nocom<'a, I>(l: &mut NoCommentIter<I>) -> Option<ParsedLine<'a>>
  where
    I: Iterator<Item = ParsedLine<'a>>,
  {
    l.next()
  }

  const COMMENTS: &'static str = "#This\n$is\n#an\n#example\nof\nsome\
                                  \nlines\n.";

  make_test!(
    works_with_slice,
    COMMENTS,
    { next_nocom, Some(pline!(4, "of", None)) };
    { next_nocom, Some(pline!(5, "some", None))}
  );

  const KEYWORD_LINES: &'static str = "#Comment\n   nokeyword\nNODE  / \
                                       \n#example\nNSMAS / \nsome\nlines\n.";

  make_test!(
    needs_no_keywords,
    KEYWORD_LINES,
    {|l: &mut NoCommentIter<_>| {
        let _ = l.next();
        let _ = l.next();
        let _ = l.next();
        let _ = l.next();
        l.skip_to_next_keyword()
      },
      None
    }
  );

  make_test!(
    finds_real_keywords,
    KEYWORD_LINES,
    { next_kw, Some(kwline!(2, b"NODE  / ", &Node)) };
    { next_kw, Some(kwline!(4, b"NSMAS / ", &Nsmas)) };
    { next_kw, None };
    { next_nocom, None }
  );

  const GES1: &'static str = "        PART 1234\
                              \n        OGRP 'hausbau'\
                              \n        DELGRP>NOD 'nix'\
                              \n        END\
                              \nNODE  / ";

  make_test!(
    can_skip_ges,
    GES1,
    {|l: &mut NoCommentIter<_>| {
        let nextline = l.next().unwrap();
        let tmp = l.skip_ges(GesNode, &nextline).unwrap();
        assert_eq!(tmp.nextline.unwrap(), pline!(4, b"NODE  / ", Some(&Node)));
        assert_eq!(tmp.skip_end, 3);
        l.next()
      }, None
    }
  );

  const GES2: &'static str = "        PART 1234\
                              \n        OGRP 'hausbau'\
                              \n        END\
                              \n        DELGRP>NOD 'nix'\
                              \n        MOD 10234\
                              \n        NOD 1 23 093402 82\
                              \n        END_MOD\
                              \n        DELELE 12\
                              \n        END";

  const GES2_NEXT: &[u8] = b"        DELGRP>NOD 'nix'";

  make_test!(
    can_skip_ges_repeatedly,
    GES2,
    {|l:  &mut NoCommentIter<_>| {
        let mut nextline = l.next().unwrap();
        let mut tmp = l.skip_ges(GesNode, &nextline).unwrap();
        assert_eq!(tmp.nextline.unwrap(), pline!(3, GES2_NEXT, None));
        assert_eq!(tmp.skip_end, 2);

        nextline = l.next().unwrap();
        tmp = l.skip_ges(GesNode, &nextline).unwrap();
        assert_eq!(tmp.nextline, None);
        assert_eq!(tmp.skip_end, 8);
        l.next()
      }, None
    }
  );

  const GES3: &'static str = "        PART 1234\
                              \n        OGRP 'hausbau'\
                              \nNODE  /         END\
                              \n        DELGRP>NOD 'nix'\
                              \n        MOD 10234\
                              \n        NOD 1 23 093402 82\
                              \n        END_MOD\
                              \nWhatever\
                              \n        END";

  const GES3_FIRST: &'static str = "NODE  /         END";
  const GES3_SECOND: &'static str = "Whatever";
  const GES3_LAST: &'static str = "        END";

  make_test!(
    ends_ges_without_end,
    GES3,
    {|l: &mut NoCommentIter<_>| {
        let mut nextline = l.next().unwrap();
        let mut tmp = l.skip_ges(GesNode, &nextline).unwrap();
        assert_eq!(tmp.nextline.unwrap(), pline!(2, GES3_FIRST, Some(&Node)));
        assert_eq!(tmp.skip_end, 1);

        nextline = l.next().unwrap();
        tmp = l.skip_ges(GesNode, &nextline).unwrap();
        assert_eq!(tmp.nextline.unwrap(), pline!(7, GES3_SECOND, None));
        assert_eq!(tmp.skip_end, 6);
        l.next()
      }, Some(pline!(8, GES3_LAST, None))
    }
  );

  const GES4: &'static str = "wupdiwup\nNODE  / ";
  const GES4_LAST: &'static str = "NODE  / ";

  make_test!(
    can_skip_empty_ges,
    GES4,
    {|l: &mut NoCommentIter<_>| {
        let nextline = l.next().unwrap();
        let tmp = l.skip_ges(GesNode, &nextline);
        assert!(tmp.is_none());
        l.next().unwrap()
      }, pline!(1, GES4_LAST, Some(&Node))
    }
  );

  const GES5: &'static str = "        PART 1234\
                              \n#Comment here\
                              \n        OGRP 'hausbau'\
                              \n        DELGRP>NOD 'nix'\
                              \n        END\
                              \n$Another comment\
                              \nNODE  / ";

  const GES5_NEXTL: &'static str = "NODE  / ";

  make_test!(
    ges_works_with_comments,
    GES5,
    {|l: &mut NoCommentIter<_>| {
        let nextline = l.next().unwrap();
        let tmp = l.skip_ges(GesNode, &nextline).unwrap();
        assert_eq!(tmp.nextline.unwrap(), pline!(6, GES5_NEXTL, Some(&Node)));
        assert_eq!(tmp.skip_end, 4);
        l.next()
      }, None
    }
 );

  const GES6: &'static str = "        PART 1234\
                              \n#Comment here\
                              \n$Another comment\
                              \n#NODE  / ";

  make_test!(
    ges_skips_comments_after_end,
    GES6,
    {|l: &mut NoCommentIter<_>| {
        let nextline = l.next().unwrap();
        let tmp = l.skip_ges(GesNode, &nextline).unwrap();
        assert_eq!(tmp.nextline, None);
        assert_eq!(tmp.skip_end, 0);
        l.next()
      }, None
    }
  );

  const CARD_MASS_INCOMPLETE: &'static str =
    "$ MASS Card\
    \n$#         IDNOD    IFRA   Blank            DISr            DISs            DISt\
    \nMASS  /        0       0                                                        \
    \n$#                                                                         TITLE\
    \nNAME MASS  / ->1                                                                \
    \n$# BLANK              Mx              My              Mz\
    \n$# BLANK              Ix              Iy              Iz                   Blank\
    \nNODE  /      \
    \n                                                        ";

  make_test!(
    skip_incomplete_cards,
    CARD_MASS_INCOMPLETE,
    {|l: &mut NoCommentIter<_>| {
        let mut folds = BufData::new();
        let firstline = l.next().unwrap();
        let tmp = l.skip_card(
          &firstline.try_into_keywordline().unwrap(),
          &MASS,
          &mut folds.highlights
        );
        assert_eq!(
          tmp.nextline.unwrap(),
          pline!(7, &"NODE  /      ", Some(&Node))
        );
        tmp.skip_end
      }, 4
    }
  );

  const LINES_GATHER: [&'static str; 20] = [
    /* 0 */
    "NODE  /        1              0.             0.5              0.",
    /* 1 */
    "NODE  /        1              0.             0.5              0.",
    /* 2 */
    "NODE  /        1              0.             0.5              0.",
    /* 3 */
    "NODE  /        1              0.             0.5              0.",
    /* 4 */
    "#Comment here",
    /* 5 */
    "SHELL /     3129       1       1    2967    2971    2970",
    /* 6 */
    "invalid line here",
    /* 7 */
    "SHELL /     3129       1       1    2967    2971    2970",
    /* 8 */
    "SHELL /     3129       1       1    2967    2971    2970",
    /* 9 */
    "#Comment",
    /* 10 */
    "#Comment",
    /* 11 */
    "SHELL /     3129       1       1    2967 2971    2970",
    /* 12 */
    "SHELL /     3129       1       1    2967    2971    2970",
    /* 13 */
    "$Comment",
    /* 14 */
    "SHELL /     3129       1       1    2967    2971    2970",
    /* 15 */
    "SHELL /     3129       1       1    2967    2971    2970",
    /* 16 */
    "$Comment",
    /* 17 */
    "#Comment",
    /* 18 */
    "NODE  /        1              0.             0.5              0.",
    /* 19 */
    "NODE  /        1              0.             0.5              0.",
  ];

  #[test]
  fn skips_gather_cards() {
    let mut folds = BufData::new();
    let keywords: Vec<_> = LINES_GATHER
      .iter()
      .map(|l| Keyword::parse(l.as_ref()))
      .collect();
    let mut li = (0i64..)
      .zip(LINES_GATHER.iter().zip(keywords.iter()))
      .map(|(n, (t, k))| ParsedLine {
        number: n,
        text: t.as_ref(),
        keyword: k.as_ref(),
      })
      .remove_comments();
    let firstline = li.next().unwrap();

    let mut tmp = li.skip_fold(
      &(firstline.try_into_keywordline()).unwrap(),
      &mut folds.highlights,
    );
    let mut tmp_nextline = tmp.nextline.unwrap();
    assert_eq!(tmp_nextline, pline!(5, &LINES_GATHER[5], Some(&Shell)));
    assert_eq!(tmp.skip_end, 3);

    tmp = li.skip_fold(
      &tmp_nextline.try_into_keywordline().unwrap(),
      &mut folds.highlights,
    );
    tmp_nextline = tmp.nextline.unwrap();
    assert_eq!(tmp_nextline, pline!(6, &LINES_GATHER[6], None));
    assert_eq!(tmp.skip_end, 5);

    let skipped = li.skip_to_next_keyword().unwrap();
    tmp = li.skip_fold(&skipped.into(), &mut folds.highlights);
    tmp_nextline = tmp.nextline.unwrap();
    assert_eq!(tmp_nextline, pline!(18, &LINES_GATHER[18], Some(&Node)));
    assert_eq!(tmp.skip_end, 15);

    tmp = li.skip_fold(
      &tmp_nextline.try_into_keywordline().unwrap(),
      &mut folds.highlights,
    );
    assert_eq!(tmp.nextline, None);
    assert_eq!(tmp.skip_end, 19);
  }

}
