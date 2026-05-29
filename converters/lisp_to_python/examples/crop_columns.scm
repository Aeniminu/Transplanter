(use transplanter)

(define (main)
  (clear)
  (do-a-flip)
  (loop
    (for i 0 6
      (if (can-harvest)
          (begin
            (harvest)
            (plant (entity carrot)))
          (begin
            (if (!= (get-ground-type) (ground soil))
                (till))
            (plant (entity carrot))))
      (move :north))
    (move :east)))
