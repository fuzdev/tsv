<script lang="ts">
	// At exactly 100 the whole alias stays inline, comment between element and suffix
	type A  =  Ffffffffffff< Aaaaaaaaaaaaaaaaaaaaaaaa , Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb >  /* c */  [];
	// At 101 the type arguments break, the alias hugs `=`, and the comment rides the closer line
	type B = Ffffffffffff< Aaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb > /* c */ [];
	// Same layout without the comment - it changes nothing about the wrapping
	type C = Ffffffffffff< Aaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb >[];
</script>
