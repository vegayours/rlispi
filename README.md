# RLispi - Lisp interpreter written in Rust.

This is a simple project written to educate myself in Rust and Lisp.
As beginner both in Rust and in Lisp (I in a process of learning Clojure) this has many rough edges and potentially bad design choices.

## Implementation details
This is an interpreter (so it is rather slow) and supports a small set of functions.
Both interactive (REPL) and 'execute script' options are supported.
Core constructs: 
- `(if cond true_branch [false_branch])`
- `(def symbol value)`
- `(import "filename")`
- `(fn (arg1 arg2 ...) body)`
Lists are represented as persistent linked lists.
List functions: `head`, `rest`, `list`, `cons`, `empty?`.

User-defined functions support tail call optimisation using `recur`:
```
(def foldl
     (fn (fun acc coll)
         (if (empty? coll)
             acc
             (recur fun (fun acc (first coll)) (rest coll))
             )))
```

## Potential further improvements
- Support macros (it is Lisp in the end!)
- Support lazy evaluation (currently everything is eagerly evaluated) so we can create infinite sequences.
- Implement IO side-effects functions (e.g. print, read, etc...)
- Add tests (yep, no tests so far =/)
- Better string support
