;;; Directory Local Variables            -*- no-byte-compile: t -*-
;;; For more information see (info "(emacs) Directory Variables")

;; This repository has no Cargo.toml at its root: every crate is a separate
;; workspace. rust-analyzer therefore finds nothing to load here and instead
;; walks up the directory tree, adopting whichever outer workspace happens to
;; contain the checkout. Every file in the repository is then reported as not
;; belonging to any project, and completion, hover and diagnostics all go
;; silent.
;;
;; The project list below is the Emacs counterpart of the
;; `rust-analyzer.linkedProjects' settings in .vscode/settings.json and
;; .zed/settings.json, and is spelled twice so that it applies whether Eglot or
;; lsp-mode is driving the server. To work on one of the crates under
;; examples/, add its Cargo.toml to both lists.

((nil
  . ((eglot-workspace-configuration
      . (:rust-analyzer
         (:linkedProjects ["rmk/Cargo.toml"
                           "rmk-config/Cargo.toml"
                           "rmk-macro/Cargo.toml"
                           "rmk-types/Cargo.toml"
                           "rynk/Cargo.toml"]
          :cargo (:noDefaultFeatures t)
          :check (:allTargets :json-false))))

     (lsp-rust-analyzer-linked-projects
      . ["rmk/Cargo.toml"
         "rmk-config/Cargo.toml"
         "rmk-macro/Cargo.toml"
         "rmk-types/Cargo.toml"
         "rynk/Cargo.toml"])
     (lsp-rust-no-default-features . t)
     (lsp-rust-analyzer-check-all-targets . nil))))
