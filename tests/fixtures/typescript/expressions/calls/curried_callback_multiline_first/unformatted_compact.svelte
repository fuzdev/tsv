<script lang="ts">
// Array arg forces multiline - second call hugs ])
fn(['aaaaaaaaaaaaaaaaaaaaa','bbbbbbbbbbbbbbbbbbbbb','ccccccccccccccccccccc','ddddddddddddddddddddd'])('a',(b) => {fn(b);});

// Member access + array arg
a.b(['aaaaaaaaaaaaaaaaaaaaa','bbbbbbbbbbbbbbbbbbbbb','ccccccccccccccccccccc','ddddddddddddddddddddd'])('a',(b) => {fn(b);});

// Object arg forces multiline - second call hugs })
fn({
a:'aaaaaaaaaa',
b:'bbbbbbbbbb',
c:'cccccccccc',
d:'dddddddddd',
})('a',(b) => {fn(b);});

// Short array - fits flat (control)
fn([1,2,3])('a',(b) => {fn(b);});
</script>
