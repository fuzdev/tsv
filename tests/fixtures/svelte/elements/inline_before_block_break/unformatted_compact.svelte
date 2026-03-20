<!-- Inline element before block element with whitespace -->
<div><span>text</span> <div>text</div></div>

<!-- Expression before block element with whitespace -->
<div>{expr} <div>text</div></div>

<!-- Component before block element with whitespace -->
<div><Comp /> <div>text</div></div>

<!-- Inline element + text before block element with whitespace -->
<div><span>text</span> text <div>text</div></div>

<!-- Multiple cases: block before inline is fine (no issue) -->
<div><div>text</div> <span>text</span></div>

<!-- Inline parent: inline element before block element -->
<span><span>text</span> <div>text</div></span>
