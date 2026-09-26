<!--
	Inside `<pre>` a special element's whitespace-only content is rendered text, so it stays as
	written, the way an inline element's or a component's does there.
-->

<pre>a<svelte:element  this="b" >   </svelte:element  >c</pre>

<pre>a<svelte:component  this={Comp} >
</svelte:component  >c</pre>

<pre>a<svelte:boundary >	</svelte:boundary  >c</pre>

<pre>a<slot >   </slot  >c</pre>

<pre>{#if cond}a<svelte:self >   </svelte:self  >c{/if}</pre>

<pre>a<Comp ><svelte:fragment  slot="name" >   </svelte:fragment  ></Comp  >c</pre>

<!-- the same content in an inline element and in a component -->
<pre>a<b >   </b  >c</pre>

<pre>a<Comp >   </Comp  >c</pre>
