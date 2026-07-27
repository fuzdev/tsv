<script>
// A line comment trailing the update forces the header open and keeps its place
for(a;b;c++ // c1
)fn();

// The same with a block body
for(a;b;c++ // c2
){fn();}

// Block comments trailing the update need no break, so the header stays on one line
for(a;b;c++ /* c3 */ /* c4 */)fn();
</script>
