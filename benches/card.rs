#![feature(test)]
extern crate test;
extern crate nvimpam_lib;

use self::test::Bencher;

use nvimpam_lib::card::keyword::Keyword;
use nvimpam_lib::folds::FoldList;
use nvimpam_lib::lines::LinesIter;
use nvimpam_lib::card::ges::GesType;

#[bench]
fn bench_parse2folddata(b: &mut Bencher) {
  use std::fs::File;
  use std::io::{self, BufRead};

  let file = File::open("files/example.pc").unwrap();
  let v: Vec<String> = io::BufReader::new(file)
    .lines()
    .map(|l| l.unwrap())
    .collect();

  let mut f = FoldList::new();
  b.iter(|| {
    let r = test::black_box(&v);
    f.clear();
    let _compacted = f.add_folds(r);
  })
}

#[bench]
fn bench_parse_str(b: &mut Bencher) {
  use std::fs::File;
  use std::io::{self, BufRead};

  let file = File::open("files/example.pc").unwrap();
  let v: Vec<String> = io::BufReader::new(file)
    .lines()
    .map(|l| l.unwrap())
    .collect();

  b.iter(|| {
    let r = test::black_box(&v);
    let _parsed: Vec<Option<Keyword>> =
      r.iter().map(|s| Keyword::parse(s)).collect();
  })
}

const GES: [&'static str; 9] = [
  "        PART 1234",
  "        OGRP 'hausbau'",
  "        END",
  "        DELGRP>NOD 'nix'",
  "        MOD 10234",
  "        NOD 1 23 093402 82",
  "        END_MOD",
  "        DELELE 12",
  "        END",
];

#[bench]
fn bench_skip_ges(b: &mut Bencher) {
    let g = GesType::GesNode;

    b.iter(|| {
      let mut itr = GES.iter().enumerate();
      let mut li = test::black_box(LinesIter { it: &mut itr });
      let mut _a = li.skip_ges(&g);
      _a = li.skip_ges(&g);
    });
}
