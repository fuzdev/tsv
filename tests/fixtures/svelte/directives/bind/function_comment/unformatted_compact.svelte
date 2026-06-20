<!-- Function-binding sequence comments are preserved at the author's position -->

<!-- Leading line comment forces the value to break; sequence stays bare (no parens) -->
<input bind:value={
// c
()=>a,(v)=>(a=v)} />

<!-- Leading line comment with a block-body setter: items break onto their own lines -->
<input bind:value={
// c
()=>a,(v)=>{a=v;}} />

<!-- Leading multi-line block comment: same broken, bare form -->
<input bind:value={
/* c1
		c2 */
()=>a,(v)=>(a=v)} />

<!-- Mid block comment (between getter and setter): preserved inline, no break -->
<input bind:value={()=>a,/* c */(v)=>(a=v)} />

<!-- Mid line comment: trails the comma and forces the break -->
<input bind:value={()=>a,// c
(v)=>(a=v)} />
