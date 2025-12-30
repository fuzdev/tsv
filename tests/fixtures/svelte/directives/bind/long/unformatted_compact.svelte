<!-- Single bind directive - element line wrapping -->

<!-- 100 chars - stays inline (at boundary) -->
<input bind:value={variableNameThatPadsToExactlySeventySixCharactersTotalForExactBoundaryTestPad} />

<!-- 101 chars - element wraps, expression on single indented line -->
<input bind:value={variableNameThatPadsToExactlySeventySixCharactersTotalForExactBoundaryTestPadX} />

<!-- Multiple bind directives - element wrapping -->

<!-- 100 chars - stays inline (at boundary) -->
<input bind:value={valueVariableAAAA} bind:checked={checkedVariableBBBB} bind:group={groupVarCCC} />

<!-- 101 chars - each binding on its own line -->
<input bind:value={valueVariableAAAAX} bind:checked={checkedVariableBBBB} bind:group={groupVarCCC} />

<!-- Expression wrapping - && chains -->

<!-- 100 chars expression line - stays on single indented line (at boundary) -->
<input bind:value={conditionA && conditionB && conditionC && conditionD && conditionE && conditionFGHIJK} />

<!-- 101 chars expression line - expression gets block structure -->
<input bind:value={conditionA && conditionB && conditionC && conditionD && conditionE && conditionFGHIJKL} />

<!-- 100 chars inside block - expression stays on single line (at boundary) -->
<input bind:value={conditionAAAAA && conditionBBBBB && conditionCCCCC && conditionDDDDD && conditionEEEEEEEEEEEEEEE} />

<!-- 101 chars inside block - expression wraps per-operator -->
<input bind:value={conditionAAAAA && conditionBBBBB && conditionCCCCC && conditionDDDDD && conditionEEEEEEEEEEEEEEEE} />

<!-- Member access expressions (chains don't break per-property like && chains) -->

<!-- 100 chars - stays inline (at boundary) -->
<input bind:value={obj.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.aa.bb.cc.dd.ee.ffggii} />

<!-- 101 chars - element wraps, chain stays on single line -->
<input bind:value={obj.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.aa.bb.cc.dd.ee.ffggiij} />
