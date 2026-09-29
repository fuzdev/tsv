<script>
	// the index's stripped pair flushes its `//` where the index ends, ahead of the comment
	// written after it, which follows on the next line
	q = x[a || (b // c1
	) // d1
	];

	// the computed key of an object property, the same seam
	r = {
		[a || (b // c2
		) // d2
		]: 1
	};
</script>

<p>{x[a || (b // c3
) // d3
]}</p>
