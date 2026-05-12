# Audit heurístico inicial — estado pre-landing

Aplicación rápida de las 10 heurísticas de Nielsen al estado actual (README como única surface web). Verde = OK, Amarillo = aceptable con caveats, Rojo = pain real.

| # | Heurística | Estado actual (README) | Color |
|---|---|---|---|
| 1 | Visibilidad del estado del sistema | Badges (CI, release, license) presentes, status v0.1.0 indicado | 🟢 |
| 2 | Match con el mundo real | Tagline en latín + español es decisión de marca, no obstáculo | 🟢 |
| 3 | Control y libertad del usuario | README permite ToC navegación pero no hay layout web | 🟡 |
| 4 | Consistencia y estándares | README sigue convenciones GitHub estándar | 🟢 |
| 5 | Prevención de errores | Donate: address copiable pero el usuario puede pegar mal | 🟡 |
| 6 | Reconocer en vez de recordar | Comandos visibles, pero install path no está agrupado visualmente | 🟡 |
| 7 | Flexibilidad y eficiencia de uso | Atajos `seele setup --agent X` documentados pero requieren scroll | 🟢 |
| 8 | Diseño estético y minimalista | README denso, sobre todo en sección Support | 🟡 |
| 9 | Ayudar a usuarios a reconocer/recuperarse de errores | N/A en README, no hay flow | — |
| 10 | Ayuda y documentación | `docs/` link presente, AGENT-SETUP claro | 🟢 |

## Conclusión del audit

El README es funcionalmente correcto pero el **donate flow tiene el peor ratio "deseo-de-donar : friction"**. Heurística 5 + heurística 8 son las que el sprint LUMEN-01 ataca directamente:

- H5 → donate widget reduce el riesgo de error de copy-paste de address.
- H8 → landing dedicada permite densidad visual menor que el README.

## Antipatrones a no replicar

Mirando landings OSS de proyectos similares (sourcegraph cody, continue.dev, langchain) y consensus entre dev influencers en 2026:

- ❌ Hero con video autoplay
- ❌ Newsletter pop-up
- ❌ Cookie banner (no usamos analytics → no necesitamos consent)
- ❌ "Get Started" CTA que lleva a un signup
- ❌ Comparison tables con competidores (ENGRAM no es competidor, es inspiración)
- ❌ Trust badges falsos ("trusted by X companies")
- ❌ Marketing speak ("revolutionary", "AI-powered", "enterprise-grade")

## Patrones a sí replicar

Influencers 2026 (Frost, Bos, Norman aplicado a OSS) coinciden:

- ✅ Tipografía monoespaciada en hero para proyectos dev
- ✅ Code block visible en primer scroll
- ✅ Bandwidth budget honesto en footer ("This page weighs X KB")
- ✅ Link directo al repo, prominente
- ✅ Pre/post-install commands ejecutables
