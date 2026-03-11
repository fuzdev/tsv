<script lang="ts">
// Block callback with shorthand object arg
const a = new Cls((x) => { fn1(x); }, {prop});

// Block callback with multi-property object arg
const b = new Cls((x) => { fn1(x); if (cond) { fn2(x); } }, {
prop1: 1,
});

// Single-statement block callback with object arg
const c = new Cls((x) => { return x; }, {prop});

// Regular call (same pattern - should match)
const d = fn((x) => { fn1(x); }, {prop});
</script>
