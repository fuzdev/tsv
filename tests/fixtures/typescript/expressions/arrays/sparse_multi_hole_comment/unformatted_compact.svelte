<script>
// Multiple line comments interleaved with elision commas
const a=[1,,,
// c1
// c2
2];

// Block comment between elision commas (multiline-forced)
const b=[1,,,
/* c */
2];

// Comment at start before two consecutive elisions
const c=[,,
// c
1];
</script>
