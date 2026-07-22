<script>
// A line comment inside the iterable array keeps the for-of head inline
// (`for (const item of [`); only the array breaks.
for(const item of['a','b'// trailing
]){fn(item);}

// A leading line comment inside the array behaves the same.
for(const item of[
// leading
'a','b']){fn(item);}

// A line comment inside a destructuring binding stays with the pattern:
// the head stays inline and only the pattern breaks.
for(const[
// leading
a,b]of pairs){fn(a,b);}

// for-in over an object iterable — same rule, shared code path.
for(const key in{a:1// trailing
}){fn(key);}
</script>
