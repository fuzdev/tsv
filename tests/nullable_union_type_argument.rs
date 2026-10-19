// helper fns here aren't `#[test]`, so clippy.toml's allow-expect-in-tests doesn't reach them
#![expect(clippy::expect_used)]

//! A sole type argument that is one type reference united only with `null` / `void`
//! (prettier's `shouldHugUnionType`) prints unbreakable wherever an enclosing break keeps the
//! line within print width, and breaks inside its `<>` only past it
//! (`docs/conformance_prettier_ts.md` §Nullable union type argument). The rows pin the
//! positions the fixtures do not reach: an assignment EXPRESSION and a class property, which
//! take the same after-`=` rule as a declarator; a member that must break, which has no
//! unbreakable form; an arrow-function variable's annotation, which prettier opts out of the
//! rule; a line already over print width before the `<`; a value wrapped around the call; the
//! after-`=` break taken on the line up to the call's `(`, at its print width boundary; a value
//! whose layout is chosen between forms from the unbroken `<>` (a longer member chain); a
//! hugging last argument, whose forms leave the `<>` outside the choice; a breakable binding
//! or type-parameter list ahead of the `=`, which stays whole while its line fits up to the
//! `=`; and either layout nested in an enclosing group (an `if` body, a call argument, an
//! arrow body, an object property, a type position), which leaves that group on one line
//! while it fits.
//! The over-width rows are tsv's own fixed point, not a prettier claim (prettier leaves the
//! line over print width there); the rows saying "prettier's form" are prettier's fixed point.

fn format(source: &str) -> String {
    tsv_ts::format_str(source).expect("formats")
}

/// Each pair is (authoring, the single-pass fixed point).
const CASES: &[(&str, &str)] = &[
    // an assignment expression breaks after `=` while the call then fits (prettier's form)
    (
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = fn<Bbbbbbbbbbbbbbbbbbbbbbbbb | null>(null);\n",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa =\n\tfn<Bbbbbbbbbbbbbbbbbbbbbbbbb | null>(null);\n",
    ),
    // ... and keeps the `=` line, breaking inside the `<>`, where not even the line up to the
    // call's `(` fits after it
    (
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaa = fn<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>(null);\n",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaa = fn<\n\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n>(null);\n",
    ),
    // a class property: `new` after `=` (prettier's form) ...
    (
        "class C {\n\taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = new Bbbbbbbbbbbbbbbbbbbbbbbbbb<Cccccc | null>();\n}\n",
        "class C {\n\taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa =\n\t\tnew Bbbbbbbbbbbbbbbbbbbbbbbbbb<Cccccc | null>();\n}\n",
    ),
    // ... and inside the `<>` past print width
    (
        "class C {\n\taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = new Bbbbbbbbbbbbbbbbbbbbbbbbbbbb<Cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc | null>();\n}\n",
        "class C {\n\taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = new Bbbbbbbbbbbbbbbbbbbbbbbbbbbb<\n\t\tCccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc | null\n\t>();\n}\n",
    ),
    // a member that must break (an authored-expanded object in its type arguments) has no
    // unbreakable form: the `<>` keeps its ordinary group
    (
        "type Aaaaaaaaaaaaaaaaaaaaaaaaaaaa = Promise<Result<{\n\taaaa: string;\n}, Eeee> | null>;\n",
        "type Aaaaaaaaaaaaaaaaaaaaaaaaaaaa = Promise<\n\tResult<\n\t\t{\n\t\t\taaaa: string;\n\t\t},\n\t\tEeee\n\t> | null\n>;\n",
    ), // an arrow-function variable's annotation takes the ordinary group (prettier's
    // `isArrowFunctionVariable`): its `<>` breaks ahead of the `=` (prettier's form) ...
    (
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<Bbbb | null> = () => null;\n",
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<\n\tBbbb | null\n> = () => null;\n",
    ),
    // ... for a `void` member too
    (
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<void | Bbbb> = () => null;\n",
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<\n\tvoid | Bbbb\n> = () => null;\n",
    ),
    // ... and for an argument that hugs anywhere else, a simple one (prettier's form) ...
    (
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<Bbbb> = () => null;\n",
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<\n\tBbbb\n> = () => null;\n",
    ),
    // ... or a union with an object type literal (prettier's form)
    (
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<{ a: string } | null> = () => {\n\treturn 1;\n};\n",
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<\n\t{ a: string } | null\n> = () => {\n\treturn 1;\n};\n",
    ),
    // where the line is over print width before the `<`, the `<>` still breaks inside: in an
    // annotation ...
    (
        "let aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<Map<Kk, Vv> | null>;\n",
        "let aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<\n\tMap<Kk, Vv> | null\n>;\n",
    ),
    // ... a `declare` binding ...
    (
        "declare const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<void | Map<Kk, Vv>>;\n",
        "declare const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<\n\tvoid | Map<Kk, Vv>\n>;\n",
    ),
    // ... a property signature ...
    (
        "interface I {\n\taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<null | Map<Kk, Vv>>;\n}\n",
        "interface I {\n\taaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<\n\t\tnull | Map<Kk, Vv>\n\t>;\n}\n",
    ),
    // ... a return type ...
    (
        "function aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(): Promise<Map<Kk, Vv> | null> {}\n",
        "function aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa(): Promise<\n\tMap<Kk, Vv> | null\n> {}\n",
    ),
    // ... and an `implements` clause
    (
        "class aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa implements Foo<Map<Kk, Vv> | null> {}\n",
        "class aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa implements Foo<\n\tMap<Kk, Vv> | null\n> {}\n",
    ),
    // a wrapped value that cannot fit after the `=` keeps the `=` line and breaks inside the
    // `<>`, as a bare call does: under `await` ...
    (
        "async function f() {\n\tconst x = await fn<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>(null);\n}\n",
        "async function f() {\n\tconst x = await fn<\n\t\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n\t>(null);\n}\n",
    ),
    // ... `as` ...
    (
        "const x = fn<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>(null) as Foo;\n",
        "const x = fn<\n\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n>(null) as Foo;\n",
    ),
    // ... `satisfies` ...
    (
        "const x = fn<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>(null) satisfies Foo;\n",
        "const x = fn<\n\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n>(null) satisfies Foo;\n",
    ),
    // ... an arrow's return type ...
    (
        "const x = (): Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null> => null;\n",
        "const x = (): Promise<\n\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n> => null;\n",
    ),
    // ... the head of a two-call member chain ...
    (
        "const x = useState<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>(null).then((x) => x);\n",
        "const x = useState<\n\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n>(null).then((x) => x);\n",
    ),
    // ... and a non-null `!`
    (
        "const x = fn<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>(null)!;\n",
        "const x = fn<\n\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n>(null)!;\n",
    ),
    // an arrow with parameters breaks them first, as an ordinary arrow does
    (
        "const x = (a: string): Promise<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null> => null;\n",
        "const x = (\n\ta: string\n): Promise<\n\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n> => null;\n",
    ),
    // an arrow-function variable's sole object-type argument hugs as anywhere (prettier's
    // form: `isArrowFunctionVariable` exempts it)
    (
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<{ a: string }> = () => null;\n",
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Foo<{\n\ta: string;\n}> = () => null;\n",
    ),
    // the after-`=` break is taken while the line up to the call's `(` fits after the `=`, the
    // arguments breaking below it (prettier's form) ...
    (
        "function Component() {\n\tconst [selectedConversationThreadItem, setSelectedConversationThreadItem] = useState<ConversationThreadItem | null>(getInitialThreadItem(props.conversationThreadId, props.itemId));\n}\n",
        "function Component() {\n\tconst [selectedConversationThreadItem, setSelectedConversationThreadItem] =\n\t\tuseState<ConversationThreadItem | null>(\n\t\t\tgetInitialThreadItem(props.conversationThreadId, props.itemId)\n\t\t);\n}\n",
    ),
    // ... at the edge too, where only the arguments' tail would overflow (prettier's form)
    (
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = fn<Bbbbbbbbbbbbbbbbbbbbbbbbb | null>(null);\n",
        "const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa =\n\tfn<Bbbbbbbbbbbbbbbbbbbbbbbbb | null>(null);\n",
    ),
    // a member chain whose `<>` fits after the `=` breaks there and expands (prettier's form) ...
    (
        "async function load() {\n\tconst [conversationThreadItems, setConversationThreadItems] = fetchJson<ConversationThreadItemList | null>(endpoint).then(parseTheThreadItemsFromTheResponse).catch(reportTheFetchErrorToTheUser).finally(done);\n}\n",
        "async function load() {\n\tconst [conversationThreadItems, setConversationThreadItems] =\n\t\tfetchJson<ConversationThreadItemList | null>(endpoint)\n\t\t\t.then(parseTheThreadItemsFromTheResponse)\n\t\t\t.catch(reportTheFetchErrorToTheUser)\n\t\t\t.finally(done);\n}\n",
    ),
    // ... with three calls as with four (prettier's form)
    (
        "async function load() {\n\tconst [conversationThreadItems, setConversationThreadItems] = fetchJson<ConversationThreadItemList | null>(endpoint).then(parseThreadItems).catch(reportFetchError);\n\tconst [conversationThreadItems, setConversationThreadItems] = fetchJson<ConversationThreadItemList | null>(endpoint).then(parseTheThreadItemsFromTheResponse).catch(reportFetchError).finally(done);\n}\n",
        "async function load() {\n\tconst [conversationThreadItems, setConversationThreadItems] =\n\t\tfetchJson<ConversationThreadItemList | null>(endpoint)\n\t\t\t.then(parseThreadItems)\n\t\t\t.catch(reportFetchError);\n\tconst [conversationThreadItems, setConversationThreadItems] =\n\t\tfetchJson<ConversationThreadItemList | null>(endpoint)\n\t\t\t.then(parseTheThreadItemsFromTheResponse)\n\t\t\t.catch(reportFetchError)\n\t\t\t.finally(done);\n}\n",
    ),
    // a value whose layout is chosen between forms from the unbroken `<>` cannot take the broken
    // one in their place: past print width a member chain of more than two calls breaks after the
    // `=` as prettier does, and inside the `<>` too (every line fits but the member's own) ...
    (
        "const x = fn<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>(null).then(y).catch(z).finally(w);\n",
        "const x =\n\tfn<\n\t\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n\t>(null)\n\t\t.then(y)\n\t\t.catch(z)\n\t\t.finally(w);\n",
    ),
    // a call whose last argument hugs keeps the `=` line as a call with any other argument does,
    // the `<>` broken inside and the hug taken after the broken `>` as after any broken
    // type-argument list (prettier breaks after the `=` and leaves the `<>` line over width)
    (
        "const x = useState<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>({ aaaaaaaaaaaa: 1, bbbbbbbbbbbbbbbbbbbbb: 2, cccccccccccccccccccccc: 3, dddddddddddddddddd: 4, eeeeeeeeeeeeee: 5 });\n",
        "const x = useState<\n\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n>({\n\taaaaaaaaaaaa: 1,\n\tbbbbbbbbbbbbbbbbbbbbb: 2,\n\tcccccccccccccccccccccc: 3,\n\tdddddddddddddddddd: 4,\n\teeeeeeeeeeeeee: 5\n});\n",
    ),
    // a lone nullable union past print width breaks inside the `<>` and the last argument hugs
    // after the `>`: at statement position (prettier leaves the `<>` line over width and breaks
    // the argument out) ...
    (
        "fn<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null>({ a: 1 });\n",
        "fn<\n\tAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null\n>({ a: 1 });\n",
    ),
    // ... and after the `=`, which stays on the `=` line as the hugging call above does
    (
        "const a = fn<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null>({ a: 1 });\n",
        "const a = fn<\n\tAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null\n>({ a: 1 });\n",
    ),
    // the member access past a call keeps the `=` line, as the call alone does ...
    (
        "const x = fn<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>(null).data;\n",
        "const x = fn<\n\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n>(null).data;\n",
    ),
    // ... as does a member chain headed by a `new` expression ...
    (
        "const x = new Foo<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>().bar();\n",
        "const x = new Foo<\n\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n>().bar();\n",
    ),
    // ... or by a non-null `!`
    (
        "const x = fn<Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null>(null)!.then(y);\n",
        "const x = fn<\n\tBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | null\n>(null)!.then(y);\n",
    ),
    // a plain union at the head boundary: the line up to the call's `(` fits after the `=` at
    // exactly print width, so it breaks there and the arguments break below (prettier's form) ...
    (
        "const aaaaaaaa = fn<BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB | null>(ccccccccccccccccccccccccc, dddddddddddddddddd);\n",
        "const aaaaaaaa =\n\tfn<BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB | null>(\n\t\tccccccccccccccccccccccccc,\n\t\tdddddddddddddddddd\n\t);\n",
    ),
    // ... and one column past it keeps the `=` line, breaking inside the `<>`
    (
        "const aaaaaaaa = fn<BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB | null>(ccccccccccccccccccccccccc, dddddddddddddddddd);\n",
        "const aaaaaaaa = fn<\n\tBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB | null\n>(ccccccccccccccccccccccccc, dddddddddddddddddd);\n",
    ),
    // a breakable binding whose line fits up to the `=` stays whole, the value breaking after
    // the `=`, though the line up to the `<` would not fit: an array pattern ...
    (
        "function ConversationSidebar() {\n\tconst [selectedConversationThreadItemIdentifier, setSelectedConversationThreadItemIdent] = useState<ThreadItemId | null>(null);\n}\n",
        "function ConversationSidebar() {\n\tconst [selectedConversationThreadItemIdentifier, setSelectedConversationThreadItemIdent] =\n\t\tuseState<ThreadItemId | null>(null);\n}\n",
    ),
    // ... an object pattern ...
    (
        "const { conversationThreadItems, selectedConversationThreadItem, isLoadingTheThreadItemsNow } = useQuery<ThreadItemsResult | null>(threadItemsQuery);\n",
        "const { conversationThreadItems, selectedConversationThreadItem, isLoadingTheThreadItemsNow } =\n\tuseQuery<ThreadItemsResult | null>(threadItemsQuery);\n",
    ),
    // ... a destructuring assignment expression ...
    (
        "[conversationThreadItemsForTheSidebarPanelNow, selectedConversationThreadItemValueForPanel] = useState<ThreadItemsResult | null>(null);\n",
        "[conversationThreadItemsForTheSidebarPanelNow, selectedConversationThreadItemValueForPanel] =\n\tuseState<ThreadItemsResult | null>(null);\n",
    ),
    // ... and a type alias's type-parameter list (prettier's form throughout)
    (
        "type ConversationThreadStore<TItemType, PPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPP> = ConversationThreadContainer<ConversationItem | null>;\n",
        "type ConversationThreadStore<TItemType, PPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPP> =\n\tConversationThreadContainer<ConversationItem | null>;\n",
    ),
    // where not even the line up to the `<` fits on the binding's line, the value breaks after
    // the `=` and inside the `<>`, the binding still whole
    (
        "function f() {\n\tconst [vxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx, setvxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx] = useState<Tyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy | null>(null);\n}\n",
        "function f() {\n\tconst [vxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx, setvxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx] =\n\t\tuseState<\n\t\t\tTyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy | null\n\t\t>(null);\n}\n",
    ),
    // an assignment nested in an enclosing group stays on one line while it fits, the group
    // whole (prettier's form): an `if` body ...
    (
        "if (open) selected = useState<ThreadItem | null>(null);\n",
        "if (open) selected = useState<ThreadItem | null>(null);\n",
    ),
    // ... an array element ...
    (
        "items = [(selected = useState<ThreadItem | null>(null))];\n",
        "items = [(selected = useState<ThreadItem | null>(null))];\n",
    ),
    // ... a call argument ...
    (
        "register((selected = useState<ThreadItem | null>(null)));\n",
        "register((selected = useState<ThreadItem | null>(null)));\n",
    ),
    // ... a `while` head ...
    (
        "while ((selected = useState<ThreadItem | null>(null))) {}\n",
        "while ((selected = useState<ThreadItem | null>(null))) {}\n",
    ),
    // ... a conditional branch ...
    (
        "x = ready ? (selected = useState<ThreadItem | null>(null)) : 0;\n",
        "x = ready ? (selected = useState<ThreadItem | null>(null)) : 0;\n",
    ),
    // ... an arrow body (a Svelte `onclick={() => (a = …)}` handler prints through the same path) ...
    (
        "const reset = () => (selected = fn<ThreadItem | null>(null));\n",
        "const reset = () => (selected = fn<ThreadItem | null>(null));\n",
    ),
    // ... an arrow argument ...
    (
        "register(1, () => (selected = useState<ThreadItem | null>(null)));\n",
        "register(1, () => (selected = useState<ThreadItem | null>(null)));\n",
    ),
    // ... a nested object's property ...
    (
        "const store = { inner: { selected: useState<ThreadItem | null>(null) } };\n",
        "const store = { inner: { selected: useState<ThreadItem | null>(null) } };\n",
    ),
    // ... a hugged object argument's property ...
    (
        "register(store, { selected: useState<ThreadItem | null>(null) });\n",
        "register(store, { selected: useState<ThreadItem | null>(null) });\n",
    ),
    // ... a `for` head ...
    (
        "for (let selected = useState<ThreadItem | null>(null); ; ) {}\n",
        "for (let selected = useState<ThreadItem | null>(null); ;) {}\n",
    ),
    // ... and a sequence
    (
        "x = ((selected = useState<ThreadItem | null>(null)), 1);\n",
        "x = ((selected = useState<ThreadItem | null>(null)), 1);\n",
    ),
    // so does the type-argument `<>` itself, nested in a group: an `if` body ...
    (
        "if (open) register(useState<ThreadItem | null>(null));\n",
        "if (open) register(useState<ThreadItem | null>(null));\n",
    ),
    // ... a call argument ...
    (
        "register(useState<ThreadItem | null>(null), selected);\n",
        "register(useState<ThreadItem | null>(null), selected);\n",
    ),
    // ... an arrow body ...
    (
        "const read = () => useState<ThreadItem | null>(null).value;\n",
        "const read = () => useState<ThreadItem | null>(null).value;\n",
    ),
    // ... a conditional branch ...
    (
        "x = ready ? useState<ThreadItem | null>(null) : selected;\n",
        "x = ready ? useState<ThreadItem | null>(null) : selected;\n",
    ),
    // ... an array element ...
    (
        "items = [useState<ThreadItem | null>(null), selected];\n",
        "items = [useState<ThreadItem | null>(null), selected];\n",
    ),
    // ... a `new` argument ...
    (
        "register(new Store<ThreadItem | null>(selected));\n",
        "register(new Store<ThreadItem | null>(selected));\n",
    ),
    // ... a method's return type in an object argument ...
    (
        "register({\n\tload(): Promise<ThreadItem | null> {\n\t\treturn x;\n\t}\n});\n",
        "register({\n\tload(): Promise<ThreadItem | null> {\n\t\treturn x;\n\t}\n});\n",
    ),
    // ... an arrow parameter's annotation ...
    (
        "register((item: Promise<ThreadItem | null>) => item);\n",
        "register((item: Promise<ThreadItem | null>) => item);\n",
    ),
    // ... an arrow's return type ...
    (
        "register((): Promise<ThreadItem | null> => item);\n",
        "register((): Promise<ThreadItem | null> => item);\n",
    ),
    // ... a property type inside another type argument ...
    (
        "let store: Foo<{ a: Bar<ThreadItem | null> }>;\n",
        "let store: Foo<{ a: Bar<ThreadItem | null> }>;\n",
    ),
    // ... a call's own type arguments ...
    (
        "register<Bar<ThreadItem | null>>(selected);\n",
        "register<Bar<ThreadItem | null>>(selected);\n",
    ),
    // ... and an `as` type
    (
        "register(selected as Bar<ThreadItem | null>);\n",
        "register(selected as Bar<ThreadItem | null>);\n",
    ),
];

#[test]
fn nullable_union_type_argument_reaches_the_fixed_point() {
    for (authored, fixed_point) in CASES {
        let once = format(authored);
        assert_eq!(once, *fixed_point, "first pass for {authored:?}");
        assert_eq!(format(&once), once, "not idempotent for {authored:?}");
    }
}
