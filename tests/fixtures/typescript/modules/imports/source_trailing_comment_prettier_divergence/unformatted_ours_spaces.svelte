<script>
	// block comment between source and `;` trails after the `;`
	import   {  a  } from './a'    /* c */   ;

	// line comment trails after `;`; own-line `// 2` stays with this statement
	import   {  b  } from './b'    // 1
	// 2
	;

	// block comment after import attributes trails after the `;`
	import   c from './c' with {  type  : 'json'  }    /* c */   ;
</script>
