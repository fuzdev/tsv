<!-- Expression content -->
<pre>{x}</pre>
<pre>text1 {x} text2</pre>
<pre class="class1">{expr}</pre>

<!-- Nested elements -->
<pre><span>text</span></pre>
<pre><code>{x}</code></pre>

<!-- Comments -->
<pre><!-- comment --></pre>
<pre>text1 <!-- comment --> text2</pre>

<!-- Textarea (same handling) -->
<textarea>{value}</textarea>
<textarea>text1 {value} text2</textarea>

<!-- Multiline with expressions -->
<pre>  text1
  {x}
  text2</pre>

<!-- Multiple expressions -->
<pre>{a} {b} {c}</pre>

<!-- Svelte blocks -->
<pre>{#if cond}text{/if}</pre>
<pre>{#each items as item}{item}{/each}</pre>
<pre>{@html content}</pre>
