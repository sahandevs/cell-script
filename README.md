### Example

```
# Title: Example 1

param math_score;
param physics_score;
param data_structure_score;


cell math:
  math_score * 3
;

cell physics:
  physics_score * 3
;

cell data_structure:
  data_structure_score * 2
;

cell total:
   math + physics + data_structure
;
```

```sh
./cell-script app.cell \
   --param math_score="10,11,13" \
   --param physics_score="15" \
   --param data_structure_score="15" \
   --query "total" \
   --format "json"
```

```json
[
  {
    "input": {
      "math_score": 10,
      "physics_score": 15,
      "data_structure_score": 15
    },
    "output": {
      "total": 105
    }
  },
  {
    "input": {
      "math_score": 11,
      "physics_score": 15,
      "data_structure_score": 15
    },
    "output": {
      "total": 109
    }
  },
  {
    "input": {
      "math_score": 11,
      "physics_score": 15,
      "data_structure_score": 15
    },
    "output": {
      "total": 113
    }
  }
]
```

### Grammar

```
S: (Param | Cell)*

Param: PARAM Ident SemiColon

Cell: CELL Ident Colon Exp SemiColon

Expr:
    | ParOpen Expr ParClose
    | Expr Plus Expr
    | Expr Sub Expr
    | Expr Mul Expr
    | Expr Div Expr
    | Atom

Atom:
    | Number
    | Ident

```

### Roadmap

- [x] scanner
- [x] parser
- [x] AST interpreter
- [x] detect cyclic dependency
- [ ] CLI
- [ ] multi-threaded
- [ ] Bytecode
- [ ] ByteCode interpreter
- [ ] Compiler / CodeGen (LLVM)
