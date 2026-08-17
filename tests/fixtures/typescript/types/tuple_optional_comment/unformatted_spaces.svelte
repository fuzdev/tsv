<script lang="ts">
	// Comments between a tuple element and its `?` marker stay in place
	type A = [ T   /* c1 */ ? ];
	type B = [ ( T & U )   /* c2 */ ? ];
	type C = [ T   /* c3 */ ? ,   U ? ];
	type D = [ T [ ]   /* c4 */ ? ];
	type E = [ T   /* c5 */   /* c6 */ ? ];

	// The gap after the `?` is a separate landing
	type F = [ T ?   /* c7 */ ];
</script>
