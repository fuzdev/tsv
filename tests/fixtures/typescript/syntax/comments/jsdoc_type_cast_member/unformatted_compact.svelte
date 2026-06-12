<script>
// member access on type-cast parenthesized expression
const a=/** @type {A} */expr.prop;
const b=/** @type {A} */expr.method(arg);

// chained member access
const c=/** @type {A} */expr.prop.nested;
const d=/** @type {A} */expr.method(arg).chained;

// with unary operators
const e=!(/** @type {boolean} */expr.prop);
const f=typeof(/** @type {A} */expr.method);
</script>
