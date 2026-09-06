<script lang="ts">
	// A sole `function` argument whose parameters are not all plain identifiers keeps a
	// breakable parameter list inside the hug (prettier's `printFunction` flattens the list
	// only past that gate), and its body's forced break makes the hug the FIRST state the
	// call offers - so a chain measuring the call sees the hug and stops at the parameter
	// list's own break, rather than reading the whole signature flat.

	// 100 - the merged short chain, everything on one line
	vi.spyOn(ctx,'fill').mockImplementation(function(this: Cccccccccccccccccccccccc,...args: any){body();
	});

	// 101 - the hug holds, the parameters break
	vi.spyOn(ctx,'fill').mockImplementation(function(this: Ccccccccccccccccccccccccc,...args: any){body();
	});
	vi.spyOn(ctx,'fill').mockImplementation(function({dddd,eeee}: Ccccccccccccccccccccccccccccc){body();
	});

	// an EMPTY body carries no forced break, so the call keeps its inline state; the
	// short chain is still a plain group, whose renderer walks that ladder to the hug
	vi.spyOn(ctx,'fill').mockImplementation(function(this: Cccccccccccccccccccccccccccc,...args: any){});

	// the long chain under an assignment stays flat for the same reason
	xxxx=vi.spyOn(ctx,'fill').mockImplementation(function(this: Cccccccccccccccccc,...args: any){body();
	});

	// all-plain parameters render flat, so the hug cannot fit and the argument breaks out
	vi.spyOn(ctx,'fill').mockImplementation(function(ddddddddCcccccccccccccccccc,ffffffffffffffff){body();
		}
	);

	// a second argument flattens the list too
	vi.spyOn(ctx,'fill').mockImplementation(aaaa,function(dddd: Ccccccccccccccccccccc,ffff: Gggg){body();
		}
	);
</script>
