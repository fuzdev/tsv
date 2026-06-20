<!-- A wide inline `<svelte:element>` too wide to share a line drops to its own line; the
     preceding word (`and`) stays hugged on the text line. Pins that the flow boundary applies
     to special elements too, the same as HTML inline elements and components. -->
<p>
	text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11 text12 aaaaaaaaaaaaaa and <svelte:element this={tag} class="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">content</svelte:element> tail1 tail2 tail3 tail4
</p>
