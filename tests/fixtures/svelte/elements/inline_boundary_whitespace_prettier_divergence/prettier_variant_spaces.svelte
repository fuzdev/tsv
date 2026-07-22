<!-- Leading space trims -->
<span> text</span>

<!-- Trailing space trims -->
<span>text </span>

<!-- Both boundary spaces trim -->
<span> text </span>

<!-- Tab run trims -->
<span> text</span>

<!-- Multiple words, boundary spaces trim -->
<span> text1 text2 </span>

<!-- With expression, leading boundary trims -->
<span> → {expr}ms</span>

<!-- With expression, trailing boundary trims -->
<span>{expr}ms → </span>

<!-- Small element -->
<small> text</small>

<!-- Nested inline element at the boundaries -->
<span> <strong>text</strong> </span>

<!-- Whitespace-only element collapses -->
<b> </b>

<!-- Whitespace-only element with attribute collapses -->
<b data-attr="value"> </b>

<!-- Comment at leading boundary: the edge space trims, the inter-sibling space stays -->
<span> <!-- c --> text</span>

<!-- Comment at trailing boundary: the inter-sibling space stays, the edge space trims -->
<span>text <!-- c --> </span>

<!-- NBSP is content, never trimmed -->
<span>  text  </span>

<!-- Inter-sibling spaces around a nested inline element stay -->
<p>text1 <span> inline1 </span> text2</p>

<!-- Expression at start, leading space trims -->
<span> {expr}</span>

<!-- Expression at end, trailing space trims -->
<span>{expr} </span>
