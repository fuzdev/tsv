<script>
// inline between properties
const {a,/* inline */b}=x;
const {/* leading */c,d}=y;
const [e,/* inline */f]=z;
const [/* leading */g,h]=w;

// nested destructuring
const {i:{/* nested */j}}=v;

// with defaults
const {k=/* default */1}=u;

// with renaming
const {l:/* rename */m}=t;

// rest element
const {n,/* rest */...o}=s;

// multiple comments
const {p,/* c1 *//* c2 */q}=r;
</script>
