<script>
// for-condition with && that fits on one line
for(let i=a;i!==b.c&&(d<e||typeof f.g==='string');i=i.h){fn();}

// for-condition 100 boundary — condition line is exactly 100 chars, stays flat
for(let i=0;a<bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb&&cccccccccccccccccccccc===ddddddddddddddddddddddddddddd;i++){fn();}

// for-condition 101 boundary — condition line is 101 chars, breaks
for(let i=0;a<bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb&&cccccccccccccccccccccc===ddddddddddddddddddddddddddddd;i++){fn();}

// for-condition with || that fits
for(let i=0;a<b||c<d;i++){fn();}

// for-condition with mixed && (non-logical comparison stays grouped)
for(let i=0;a<b&&c===d;i++){fn();}
</script>
