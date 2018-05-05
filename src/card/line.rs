//! An enum to classify the several types of lines that can occur inside a card
//! of a Pamcrash input file. Might not really be a line (see GES).
use card::cell::Cell;
use card::ges::GesType;

/// A line inside a card in a Pamcrash input file.
#[derive(Debug, PartialEq)]
pub enum Line {
  /// A standard line, containing several cells of a fixed width
  Cells(&'static [Cell]),
  /// A General Entity Selection, can consist of several lines
  Ges(GesType),
  /// A line that provides a Conditional
  Provides(&'static [Cell], Conditional),
  /// An optional line, depending on a conditional
  ///
  /// The conditional is given as an index. Walking through the lines of a
  /// card will push the result of all conditionals to a Vec in order of
  /// occurence. The optional lines will index directly into the Vec to check
  /// for the result of their conditional.
  Optional(&'static [Cell], u8),
}

/// An enum to represent different conditionals on lines
#[derive(Debug, PartialEq)]
pub enum Conditional {
  /// The char at the given index (0-based!) is the given one.
  RelChar(u8, char),
}

impl Conditional {
  /// Given a line, evaluate the condition on it
  pub fn evaluate<'a, T: 'a>(&self, line: &'a T) -> bool
  where
    T: AsRef<str>,
  {
    match *self {
      Conditional::RelChar(idx, c) => {
        let idx = idx as usize;
        line.as_ref().get(idx..idx + 1) == Some(&c.to_string())
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use card::line::Conditional;

  #[test]
  fn relchar_can_be_evaluated() {
    let cond1 = Conditional::RelChar(2, 'b');
    let cond2 = Conditional::RelChar(3, 'b');
    let line = "abbxy oaslkj";

    assert!(cond1.evaluate(&line));
    assert!(!cond2.evaluate(&line));
  }

  #[test]
  fn relchar_out_of_bounds() {
    let cond1 = Conditional::RelChar(95, 'b');
    let line = "abbxy oaslkj";

    assert!(!cond1.evaluate(&line));
  }

}
