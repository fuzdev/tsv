<!-- Inline element with single-line content stays inline -->
<pre><span>text</span></pre>

<!-- Inline element with multiline content: > breaks to new line -->
<pre><span
		>text
</span></pre>

<!-- Inline element with attr and multiline content -->
<pre><span data-attr="value"
		>text
</span></pre>

<!-- Multiple inline elements with multiline content -->
<pre><span class="class1"
		>text1
</span><span class="class2"
		>text2
</span></pre>

<!-- Inline element with expression and multiline content -->
<pre><span data-attr="value"
		>{expr}
</span></pre>

<!-- Inline element with multiline content spanning multiple lines -->
<pre><span
		>line1
line2</span
	></pre>

<!-- Block with inline multiline element -->
<pre>{#if cond}<span class="class1"
			>text
</span>{/if}</pre>

<!-- Each block with inline multiline element -->
<pre>{#each items as item}<span class="class1"
			>{item}
</span>{/each}</pre>

<!-- Leading space keeps > inline (space provides natural break) -->
<pre><span class="class1"> {expr}
</span></pre>

<!-- Leading newline keeps > inline -->
<pre><code>
  text
</code></pre>
