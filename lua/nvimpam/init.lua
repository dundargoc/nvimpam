local utils = require('nvimpam.utils')
local highlight = require('nvimpam.highlight')
local fold = require('nvimpam.fold')
local job = require('nvimpam.job')

return {
  -- job
  attach = job.attach,
  detach = job.detach,
  detach_all = job.detach_all,
  on_stderr = job.on_stderr,
  on_exit = job.on_exit,
  printstderr = job.printstderr,
  -- fold
  update_folds = fold.update_folds,
  refresh_folds = fold.refresh_folds,
  foldtext = fold.foldtext,
  printfolds = fold.printfolds,
  -- utils
  locate_binary = utils.locate_binary,
  -- highlight
  highlight_region = highlight.highlight_region,
}
