<script lang="ts">
	// Declarator: the flat type argument fits on no line of the expanded chain while the hugged
	// first line fits, so the chain stays flat and the type literal breaks inside its `<…>`.
	const rows1 = db.where(filter).select<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	}>(columns);

	// A mapped type argument: the same hug.
	const rows2 = db.where(filter).select<{
		readonly [K in Keys]: string | number | boolean | bigint | symbol | null | undefined;
	}>(columns);

	// Call argument: the same hug.
	send(
		db.where(filter).select<{
			id: string;
			display_name: string;
			email_address: string;
			created_at: Date;
			updated_at: Date;
		}>(columns)
	);

	// Property value: the same hug.
	const result = {
		rows: db.where(filter).select<{
			id: string;
			display_name: string;
			email_address: string;
			created_at: Date;
			updated_at: Date;
		}>(columns)
	};

	// Return value under `await`: the same hug.
	async function load() {
		return await db.where(filter).select<{
			id: string;
			display_name: string;
			email_address: string;
			created_at: Date;
			updated_at: Date;
		}>(columns);
	}

	// A union of the literal and `null` hugs as the literal does: the same hug.
	const rows6 = db.where(filter).select<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	} | null>(columns);

	// An optional link, and no arguments: the same hug.
	const rows7 = db?.where(filter).select<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	}>();

	// A non-null link: the same hug.
	const rows_nn = db.where(filter)!.select<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	}>(columns);

	// A `new` head: the same hug.
	const rows_new = new Db().where(filter).select<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	}>(columns);

	// A statement whose short head merges into the chain's first line: the same hug.
	db.from(table).where(filter).select<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	}>(columns);

	// A last call taking an arrow: the same hug.
	const ids = rows.filter(active).map<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	}>((row) => row.id);

	// A last call taking a block-bodied callback: the callback's own break comes after the
	// type literal, so the same hug.
	const names = rows.filter(active).map<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	}>((row) => {
		return row.name;
	});

	// Lower boundary: the type argument fits flat on the expanded chain's line up to its `(`
	// (exactly 100), so the chain breaks and the type literal stays flat.
	const rows8 = db
		.where(filter)
		.select<{ id: string; name: string; email: string; kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk: Date }>(
			columns
		);

	// One char wider (101): the type argument fits on no chain line, so the chain hugs.
	const rows9 = db.where(filter).select<{
		id: string;
		name: string;
		email: string;
		kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk: Date;
	}>(columns);

	// Upper boundary: the hugged first line is exactly 100, so the chain hugs.
	const rows10 = db.where(filter).ssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssss<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	}>(columns);

	// One char wider (101): the hugged first line does not fit, so the chain breaks with the type
	// literal broken inside it.
	const rows11 = db
		.where(filter)
		.sssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssssss<{
			id: string;
			display_name: string;
			email_address: string;
			created_at: Date;
			updated_at: Date;
		}>(columns);

	// No arguments: the expanded chain's line holds the type literal through its `();` (exactly
	// 100), so the chain breaks and the type literal stays flat.
	const rows14 = db
		.where(filter)
		.select<{ id: string; name: string; email: string; kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk: Date }>();

	// One char wider (101): the chain hugs.
	const rows15 = db.where(filter).select<{
		id: string;
		name: string;
		email: string;
		kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk: Date;
	}>();

	// A comment inside an empty argument list breaks it like an argument list, so the expanded
	// chain's line ends at its `(`: at 100 the type argument fits flat on it, so the chain breaks.
	const rows23 = db
		.where(filter)
		.select<{ id: string; name: string; email: string; kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk: Date }>(
			/* all */
		);

	// One char wider (101): the chain hugs.
	const rows24 = db.where(filter).select<{
		id: string;
		name: string;
		email: string;
		kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk: Date;
	}>(/* all */);

	// A comment between the call's name and its `<` is part of the expanded chain's line: at 100
	// the type argument fits flat on it, so the chain breaks and the type literal stays flat.
	const rows16 = db
		.where(filter)
		.select/* c */ <{ id: string; name: string; email: string; kkkkkkkkkkkkkkkkkkkkkkkkkkk: Date }>(
			columns
		);

	// One char wider (101): the chain hugs.
	const rows17 = db.where(filter).select/* c */ <{
		id: string;
		name: string;
		email: string;
		kkkkkkkkkkkkkkkkkkkkkkkkkkkk: Date;
	}>(columns);

	// A comment after an optional call's `?.`, at 100: the chain breaks.
	const rows18 = db
		.where(filter)
		.select?./* c */ <{ id: string; name: string; email: string; kkkkkkkkkkkkkkkkkkkkkkkkk: Date }>(
			columns
		);

	// One char wider (101): the chain hugs.
	const rows19 = db.where(filter).select?./* c */ <{
		id: string;
		name: string;
		email: string;
		kkkkkkkkkkkkkkkkkkkkkkkkkk: Date;
	}>(columns);

	// A comment-bearing object argument, which no object-argument hug takes: the same hug.
	const rows20 = db.where(filter).select<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	}>(
		{ id: 1 } // c
	);

	// A memberless type literal has nothing to break: the chain stays flat in both.
	const rows21 = db.where(filter).select<{} | null>(columns);

	// A later call in the group with type arguments of its own: the first typed call's type
	// argument decides, the same hug.
	const rows22 = db.where(filter).select<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	}>(columns)!<Row>(key);

	// An earlier call takes a function argument: the chain breaks with the type literal broken.
	const rows12 = db
		.where((row) => row.active)
		.select<{
			id: string;
			display_name: string;
			email_address: string;
			created_at: Date;
			updated_at: Date;
		}>(columns);

	// A type literal inside a reference argument does not hug the `<`: the chain and the `<…>`
	// break, and the literal fits flat inside it.
	const rows13 = db
		.where(filter)
		.select<
			Partial<{ id: string; display_name: string; email_address: string; created_at: Date; ok: 1 }>
		>(columns);
</script>

<!-- Template expression position: the same hug, under the tag's closing brace. -->
<p>
	{db.where(filter).select<{
		id: string;
		display_name: string;
		email_address: string;
		created_at: Date;
		updated_at: Date;
	}>(columns)}
</p>
