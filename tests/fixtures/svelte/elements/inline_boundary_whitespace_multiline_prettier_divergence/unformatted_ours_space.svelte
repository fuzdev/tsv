<!-- Block child forces multiline: the element lays out block-style, tags intact -->
<span> text<div>block</div></span>

<!-- Control-flow block forces multiline -->
<span> text{#if cond}text1{/if}</span>

<!-- Boundary whitespace on both sides converges to the same form -->
<span> text1<div>block</div>text2 </span>
