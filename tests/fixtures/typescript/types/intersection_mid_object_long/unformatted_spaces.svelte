<script lang="ts">
	// Mid-object grouping: object neither first nor last stays grouped with its neighbors
	// 100 chars - stays inline (at boundary)
	type   Mid100   =   Aaaaaaaaaaaaaaaaaaaa   &   Bbbbbbbbbbbb   &   {   c:   C   }   &   Dddddddddddd   &   Eeeeeeeeeeeeeeeeeee   ;

	// 101 chars - wraps; object hugs its neighbors on the middle line
	type   Mid101   =   Aaaaaaaaaaaaaaaaaaaa   &   Bbbbbbbbbbbb   &   {   c:   C   }   &   Dddddddddddd   &   Eeeeeeeeeeeeeeeeeeee   ;
</script>
