# parameter_property_override

`override` on a constructor parameter makes it a parameter property, like `public` or `readonly`: TypeScript emits the `this.a = a` assignment. Refused (`UnsupportedKind::ParameterProperty`); the span is the modifier.

## Override

ts-blank-space does not treat `override` as a parameter-property modifier: it blanks the annotation and leaves `constructor(override a)`, which is no JS and drops the assignment. The refusal is reviewed rather than derived. Node refuses it too.
