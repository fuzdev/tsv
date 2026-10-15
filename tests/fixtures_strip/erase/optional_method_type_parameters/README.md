# optional_method_type_parameters

An optional method with type parameters: the `?`, the type parameters and the return type blank.

## Override

ts-blank-space leaves the `?` and the type parameters in place and inserts blanks before them (`m?<T>()` to `m?   <T>()`), which is no JS and lengthens the line. The expected output is reviewed rather than derived; Node writes exactly this.
