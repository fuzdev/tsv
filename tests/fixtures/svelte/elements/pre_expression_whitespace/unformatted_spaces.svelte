<!-- Expression whitespace is normalized even in pre/textarea -->
<pre>{ x }</pre>
<pre>  { x }  </pre>
<pre>text { x } text</pre>
<textarea>{ value }</textarea>

<!-- Multiple expressions -->
<pre>{ a } { b } { c }</pre>

<!-- Nested in code -->
<pre><code>{ x }</code></pre>

<!-- Svelte block tags -->
<pre>{#if  cond}text{/if}</pre>
<pre>{@html  content}</pre>
<pre>{#each  items as item}{ item }{/each}</pre>
