<script>
// Inline: two JSDoc cast args fit on one line
fn(/** @type {A} */ aaaa,/** @type {B} */ bbbb);

// Boundary at 100: stays inline
fn(/** @type {AAAAAAAAA} */ aaaaaaaaaaaa,/** @type {BBBBBBBBB} */ bbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

// Boundary at 101: args break - each cast stays with its expression
fn(/** @type {AAAAAAAAA} */ aaaaaaaaaaaa,/** @type {BBBBBBBBB} */ bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb);

// Expanded: three args with JSDoc casts
fn(/** @type {AAAAAAAAAAAA} */ aaaaaaaaaaaaa,/** @type {BBBBBBBBBBBB} */ bbbbbbbbbbbbb,cccccccccccc);

// Expanded: cast before a call expression arg
fn(/** @type {AAAAAAAAAA} */ call(aaaaaa,bbbbbb),/** @type {BBBBBBBBBB} */ call(cccccc,dddddd));

// Expanded: JSDoc cast before non-first arg only
fn(aaaaaaaaaaaaaaaaaaaaaaaa,/** @type {BBBBBBBBBBBBBBBBBBBBBBBBBBB} */ bbbbbbbbbbbbbbbbbbbbbbbbb);
</script>
