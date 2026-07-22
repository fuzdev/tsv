<script>
// A multiline block comment followed by a line comment, trailing the last
// element of an array pattern: both are preserved, the line comment following
// the block's close on the same line.
const[a,b/* line1
line2 */// trailing
]=arr;

// The same shape on the last property of an object pattern.
const{x,y/* line1
line2 */// trailing
}=obj;
</script>
