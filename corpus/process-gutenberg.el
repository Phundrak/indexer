(require 'f)

(defun delete-gutenberg()
  (interactive)
  (goto-char (point-min))
  (re-search-forward (rx "START OF "
                         (? "THIS ")
                         (? "THE ")
                         "PROJECT GUTENBERG"))
  (re-search-forward "^$")
  (delete-region (point-min) (point))
  (re-search-forward (rx "END OF "
                         ;; (? "THE ")
                         ;; (? "THIS ")
                         (? (or "THE" "THIS") " ")
                         "PROJECT GUTENBERG"))
  (beginning-of-line)
  (delete-region (point) (point-max)))

(dolist (file (f-entries "unprocessed"))
  (message "%s" file)
  (with-temp-buffer
    (insert-file-contents file)
    (delete-gutenberg)
    (f-write-text (buffer-string)
                  'utf-8
                  (expand-file-name (f-relative file "processing")
                                    "unprocessed/"))))

;; (mapcar #'f-filename (f-entries "unprocessed/"))
;; (expand-file-name)
