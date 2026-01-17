<!-- Block followed by inline on same line - space should be removed -->

<!-- Pre before inline -->
<pre>code</pre>   <span>text</span>

<!-- Div before inline -->
<div>block</div>   <span>text</span>

<!-- P before inline -->
<p>block</p>   <span>text</span>

<!-- Multiple inlines after block -->
<pre>code</pre>   <span>inline1</span><span>inline2</span>

<!-- Nested context (passes - different code path) -->
<div>
	<pre>code</pre>   <span>text</span>
</div>
