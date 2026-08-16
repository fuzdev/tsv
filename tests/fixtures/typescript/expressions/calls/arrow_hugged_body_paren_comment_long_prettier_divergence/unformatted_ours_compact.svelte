<script lang="ts">
	// a chain whose head call carries its own arguments — 100 chars, so the argument
	// stays hugged to `.g(` and the retained parens keep the comment on that line
f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,1).g((x)=>({ k: x }   /* c */));

	// 101 chars: the argument expands onto its own line, the layout that reassembles
	// the arrow around the object body
   f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,1).g( (x)=>({ k: x } /* c */) );

	// last argument of a multi-argument call — 100 chars, everything inline
f(1,(x)=>(g(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)    /* c */));

	// 101 chars: every argument takes its own line, the layout that reassembles the
	// arrow around the call body
f(1,   (x)=>(g(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) /* c */)  );
</script>
