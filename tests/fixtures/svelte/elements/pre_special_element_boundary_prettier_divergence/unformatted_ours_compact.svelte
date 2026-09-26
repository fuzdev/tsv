<!--
	Inside `<pre>` a `<svelte:boundary>` lays out like every other element of the
	whitespace-sensitive family: a line break in its content breaks its tags around the content,
	and a line too wide for it breaks inside its tags.
-->

<!-- a line break in the content: the `>` takes the next line and the closing `>` dangles -->
<pre>a<svelte:boundary>text1
  text2</svelte:boundary>c</pre>

<!-- exactly 100 columns: nothing moves -->
<pre>a<svelte:boundary>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1   text2</svelte:boundary>c</pre>

<!-- 101 columns: the closing `>` dangles -->
<pre>a<svelte:boundary>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text1   text2</svelte:boundary>c</pre>

<!-- a wrapping attribute list: the `>` hugs the last attribute and the closing `>` dangles -->
<pre>a<svelte:boundary onerror={handle_error_aaaaaaaaaaaaaaaaaaaaaaaa} failed={failed_aaaaaaaaaaaaaaaaaaaaaaa}>text1   text2</svelte:boundary>c</pre>

