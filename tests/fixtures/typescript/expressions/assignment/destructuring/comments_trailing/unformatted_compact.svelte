<script>
// trailing after last property
const {a,b/* trailing */}=x;
const [c,d/* trailing */]=y;

// nested destructuring
const {e:{f/* nested */}}=z;

// after default value
const {g=1/* trailing */}=w;

// after renamed property
const {h:i/* trailing */}=v;

// after rest element
const {...j/* trailing */}=u;

// multiple trailing
const {k/* c1 *//* c2 */}=t;

// commented out properties (original case)
const {l/*, m*/}=s;
const [n/*, o*/]=r;
</script>
