<!-- Pre between block siblings -->
<p>text</p>
<pre>code</pre>
<p>text</p>

<!-- Pre after inline (newline inserted, no space) -->
<span>text</span>   <pre>code</pre>

<!-- Pre before inline -->
<pre>code</pre>
<span>text</span>

<!-- Multiple pre elements -->
<pre>code1</pre>   <pre>code2</pre>

<!-- Pre nested in container -->
<div>
	<pre>code</pre>
</div>

<!-- Pre with expression sibling -->
<p>{expr}</p>
<pre>code</pre>
