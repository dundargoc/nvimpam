//! The events that nvimpam needs to accept and deal with. They're sent by the
//! [`NeovimHandler`](::handler::NeovimHandler) to the main loop.
use std::{ffi::OsString, fmt, sync::mpsc};

use failure::{self, Error};

use neovim_lib::{
  neovim::Neovim,
  neovim_api::{Buffer, NeovimApi},
};

use crate::{bufdata::BufData, lines::Lines};

/// The event list the main loop reacts to
pub enum Event {
  /// The update notification for a buffer change. Full lines only. Firstline
  /// is zero-indexed (i.e. a change on the first line will have `firstline =
  /// 0`). The range from firstline to lastline is end-exclusive. `more`
  /// indicates if we need to expect another event of this type with more
  /// lines, in case Neovim decided to split up the buffer (not yet
  /// implemented).
  LinesEvent {
    buf: Buffer,
    changedtick: u64,
    firstline: i64,
    lastline: i64,
    linedata: Vec<String>,
    more: bool,
  },
  /// Update notification for a new `changedtick` without a buffer change.
  /// Used by undo/redo.
  ChangedTickEvent { buf: Buffer, changedtick: u64 },
  /// Notification the liveupdates are ending. Possible causes:
  ///  - Closing all a buffer's windows (unless 'hidden' is enabled).
  ///  - Using |:edit| to reload the buffer
  ///  - reloading the buffer after it is changed from outside neovim.
  DetachEvent { buf: Buffer },
  /// Recreate and resend the folds
  RefreshFolds,
  /// Highlight lines in the buffer containing at least the given line range
  // TODO: maybe accept buffer as an argument?
  HighlightRegion { firstline: u64, lastline: u64 },
  /// This plugin should quit. Currently only sent by the user directly.
  Quit,
}

impl Event {
  /// Run the event loop. The receiver receives the events from the
  /// [handler](::handler::NeovimHandler).
  ///
  /// The loop starts by enabling
  /// [buffer events](https://neovim.io/doc/user/api.html#nvim_buf_attach()).
  /// It creates [`lines`](::lines::Lines),
  /// [`keywords`](::card::keyword::Keywords) and a
  /// [`foldlist`](::folds::FoldList)  and updates them from the events
  /// received. It calls [`resend_all`](::folds::FoldList::resend_all) when the
  /// [`foldlist`](::folds::FoldList) was created, or the
  /// [`RefreshFolds`](../event/enum.Event.html#variant.RefreshFolds) event was
  /// sent.
  ///
  /// Sending the [`Quit`](../event/enum.Event.html#variant.Quit) event will
  /// exit the loop and return from the function.
  pub fn event_loop(
    receiver: &mpsc::Receiver<Event>,
    mut nvim: Neovim,
    file: Option<OsString>,
  ) -> Result<(), Error> {
    use self::Event::*;
    use crate::card::keyword::Keywords;

    let curbuf = nvim.get_current_buf()?;

    let mut foldlist = BufData::new();
    let mut tmp_folds: BufData;
    let origlines;
    let mut lines = Default::default();
    let mut keywords: Keywords = Default::default();

    let connected = match file {
      None => curbuf.attach(&mut nvim, true, vec![])?,
      Some(f) => {
        origlines = Lines::read_file(f)?;
        lines = Lines::from_slice(&origlines);
        keywords = Keywords::from_lines(&lines);
        foldlist.recreate_all(&keywords, &lines)?;
        foldlist.resend_all_folds(&mut nvim)?;
        curbuf.attach(&mut nvim, false, vec![])?
      }
    };

    if !connected {
      return Err(failure::err_msg("Could not enable buffer updates!"));
    }

    loop {
      match receiver.recv() {
        Ok(LinesEvent {
          firstline,
          lastline,
          linedata,
          changedtick,
          ..
        }) => {
          if changedtick == 0 {
            continue;
          }

          if lastline == -1 {
            lines = Lines::from_vec(linedata);
            keywords = Keywords::from_lines(&lines);
            foldlist.recreate_all(&keywords, &lines)?;
            foldlist.resend_all_folds(&mut nvim)?;
          } else if lastline >= 0 && firstline >= 0 {
            let added: i64 = linedata.len() as i64 - (lastline - firstline);
            keywords.update(firstline as usize, lastline as usize, &linedata);
            lines.update(firstline as usize, lastline as usize, linedata);
            tmp_folds = Default::default();
            let first = keywords.first_before(firstline as u64);
            let last = keywords.first_after((lastline as i64 + added) as u64);
            tmp_folds.recreate_all(
              &keywords[first as usize..last as usize],
              &lines[first as usize..last as usize],
            )?;
            crate::bufdata::highlights::highlight_region(
              tmp_folds.highlights.iter(),
              &mut nvim,
              first as u64,
              last as u64,
              true
            )?;
            foldlist.splice(tmp_folds, first as usize, last as usize, added);
          } else {
            error!(
              "LinesEvent only works with nonnegative numbers, except for
               lastline = -1!"
            );
          }
        }
        Ok(RefreshFolds) => {
          foldlist.resend_all_folds(&mut nvim)?;
        }
        Ok(HighlightRegion {
          firstline,
          lastline,
        }) => {
          let fl = keywords.first_before(firstline);
          let mut ll = keywords.first_after(lastline);

          // highlight_region is end_exclusive, so we need to make sure
          // we include the last line requested even if it is a keyword line
          if ll == lastline {
            ll += 1;
          }

          crate::bufdata::highlights::highlight_region(foldlist.highlights.linerange(fl, ll), &mut nvim, fl,
          ll, false)?;
        }
        Ok(Quit) => {
          break;
        }
        Ok(o) => {
          warn!("receiver recieved {:?}", o);
        }
        Err(e) => {
          warn!("receiver received error: {:?}", e);
        }
      }
    }
    info!("quitting");
    Ok(())
  }
}

impl fmt::Debug for Event {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    use self::Event::*;

    match *self {
      LinesEvent {
        changedtick,
        firstline,
        lastline,
        ref linedata,
        ..
      } => write!(
        f,
        "Update{{ changedtick: {}, firstline: {}, \
         lastline: {}, #linedata: {} }}",
        changedtick,
        firstline,
        lastline,
        linedata.len()
      ),
      ChangedTickEvent { changedtick, .. } => {
        write!(f, "ChangedTick{{ changedtick: {} }}", changedtick,)
      }
      HighlightRegion {
        firstline,
        lastline,
      } => write!(
        f,
        "Hl_Line{{ firstline: {}, lastline: {} }}",
        firstline, lastline
      ),
      DetachEvent { .. } => write!(f, "UpdatesEnd"),
      RefreshFolds => write!(f, "RefreshFolds"),
      Quit => write!(f, "Quit"),
    }
  }
}
