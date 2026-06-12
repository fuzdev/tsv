<script lang="ts">
// Short - stays inline
const a = fn(aa.bb, ([c, d]) => ({e: c, f: d.f}));

// Long - triggers divergence: Prettier hugs opening + expands object, we expand args + keep object inline
const b = fn(aaaa.bbbbbbbbbbbbbbbbbbbbbbbb, ([cc, dd]) => ({ee: cc, ff: dd.ff, gg: dd.gg, hh: dd.hh}));
</script>
