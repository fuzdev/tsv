<script>
// 100 boundary - !!(chain) fits on one line
const x = !!(a.aaa||a.bbb||a.ccc||a.ddd.length>0||a.eee.length>0||a.fff.length>0);

// 101 boundary - !!(chain) breaks, each operand on own line
const y = !!(a.aaa||a.bbb||a.ccc||a.ddd.length>0||a.eee.length>0||a.ffff.length>0);

// Short - fits inline
const z = !!(a.aaa||a.bbb||a.ccc);

// && operator - same behavior at 101
const w = !!(a.aaa&&a.bbb&&a.ccc&&a.ddd.length>0&&a.eee.length>0&&a.ffff.length>0);
</script>
