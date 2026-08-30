# Refonte UI/UX, accessibilité WCAG/RGAA et réordonnancement des notes

Date : 2026-08-30
Statut : validé par l'utilisateur en conversation (options B retenues pour navigation et fusion d'actions)

## Contexte

Memo-Tori GTK est fonctionnel (capture rapide, recherche FTS5, tags, tray,
single-instance) mais l'UI accumule de la friction et des manquements
d'accessibilité identifiés lors d'un audit :

- Deux mécanismes de navigation redondants (PopoverMenuBar + StackSwitcher).
- Trois boutons pour éditer une note (Save note / Apply tags séparés).
- Textes d'interface mêlant anglais et français, exposant des détails
  d'implémentation ("Search notes (FTS5)").
- Labels de champs uniquement portés par `placeholder_text` (non fiable pour
  les lecteurs d'écran, WCAG 1.3.1 / 4.1.2, critère RGAA 11).
- Échecs d'écriture DB silencieusement ignorés (WCAG 3.3.1).
- `config.text_scale` déclaré mais jamais appliqué (WCAG 1.4.4).
- Raccourcis clavier non documentés dans l'UI (WCAG 3.2).
- Pas de moyen de réordonner manuellement les notes.

## Objectifs

1. UI dépouillée : un seul mécanisme de navigation, une seule action de
   sauvegarde par contexte.
2. Cohérence linguistique totale en français.
3. Conformité WCAG 2.1 AA / RGAA sur les points identifiés.
4. Réordonnancement manuel des notes par glisser-déposer, avec équivalent
   clavier (WCAG 2.1.1).

## Décisions validées

### Navigation — Option B (HeaderBar unique)

Remplacer `PopoverMenuBar` + `Box` de switcher par une unique
`gtk::HeaderBar` :
- `title_widget` = le `StackSwitcher` existant (Capture / Notes), centré
  nativement par GTK.
- `pack_end` : un bouton icône seule (`application-exit-symbolic`) avec
  tooltip **et** `set_accessible_role`/nom accessible "Quitter
  l'application", relié à `app.quit`.
- Le titre de fenêtre ("Memo-Tori") reste porté par `ApplicationWindow`, pas
  dupliqué dans la HeaderBar (évite la redondance visuelle).

### Actions notes — Option B (bouton fusionné)

Le panneau Notes perd les boutons "Apply tags" et "Save note" séparés, au
profit de deux boutons dans une seule rangée d'actions :
- **"Enregistrer"** (primaire) : persiste en une seule opération le contenu
  du `reader` ET les tags de `selected_tags_entry` pour la note
  sélectionnée.
- **"Annuler"** (secondaire) : recharge depuis la DB le contenu et les tags
  de la note sélectionnée, écrasant toute modification non enregistrée dans
  les widgets.

Le panneau Capture garde son fonctionnement actuel (un seul
Enregistrer/Annuler), seuls les libellés passent en français.

### Cohérence linguistique

Tous les textes visibles par l'utilisateur passent en français. Table de
correspondance (non exhaustive, complétée pendant l'implémentation) :

| Anglais actuel | Français |
|---|---|
| Save | Enregistrer |
| Cancel | Annuler |
| Save note | (supprimé, fusionné dans Enregistrer) |
| Apply tags | (supprimé, fusionné dans Enregistrer) |
| Search notes (FTS5) | Rechercher dans les notes |
| No notes yet. | Aucune note pour l'instant. |
| No note selected. | Aucune note sélectionnée. |
| Note not found. | Note introuvable. |
| Search error / Search failed | Échec de la recherche |
| Failed to load note | Échec du chargement de la note |
| Tags: error | Tags : erreur de chargement |

### Accessibilité — actions concrètes

1. **Labels visibles** : chaque champ de saisie (recherche, filtre tags,
   tags de la note, capture, éditeur de note) reçoit un `Label` visible
   juste au-dessus, en plus (pas à la place) du `placeholder_text`
   descriptif court. Le `Label` est lié via l'ordre visuel — GTK4 associe
   automatiquement un `Label` immédiatement suivi d'un widget interactif
   dans un même conteneur si on utilise `Label::set_mnemonic_widget`.
2. **Zone de statut d'erreur visible** : une `Label` dédiée
   `error_banner` (masquée par défaut, `visible=false`), affichée en rouge
   sombre sur fond clair contrasté (vérifié AA) quand une opération DB
   échoue, avec un message clair en français. Son `AccessibleRole` est
   positionné pour être annoncée par un lecteur d'écran au changement de
   texte (GTK4 : `update_property` avec
   `gtk::AccessibleProperty` n'existe pas pour "live region" directement —
   utiliser `set_accessible_role(gtk::AccessibleRole::Alert)` sur le
   `Label`, ce rôle ATK/AT-SPI est annoncé automatiquement à l'apparition).
3. **`text_scale` branché** : `install_css()` prend le facteur d'échelle en
   paramètre et génère le CSS avec un multiplicateur appliqué sur les
   `font-size` déclarées explicitement (actuellement aucune taille de
   police explicite dans le CSS — on en ajoute une base `font-size: 1rem`
   sur `*` puis on la fait varier : `calc(1rem * {scale})` n'est pas
   supporté tel quel par GTK CSS, donc on injecte directement la valeur
   calculée en Rust : `format!("font-size: {}px;", (13.0 * scale) as i32)`
   sur le sélecteur `*`).
4. **Aide raccourcis visible** : un petit texte discret
   (classe CSS `.shortcuts-hint`, couleur `--muted` déjà validée AA) sous
   la zone de capture : "Entrée : enregistrer · Échap : effacer · Ctrl+Tab :
   changer d'onglet".

### Réordonnancement des notes

- Nouvelle colonne `notes.position INTEGER NOT NULL DEFAULT 0`, ajoutée via
  migration procédurale (le schéma actuel n'a pas de vrai système de
  migrations versionnées — un seul script rejoué avec `CREATE TABLE IF NOT
  EXISTS`). Utiliser `PRAGMA table_info(notes)` en Rust pour détecter
  l'absence de la colonne et faire `ALTER TABLE notes ADD COLUMN position
  INTEGER NOT NULL DEFAULT 0` une seule fois, puis backfill les valeurs
  existantes par `updated_at DESC` (les plus récentes en position basse,
  cohérent avec l'ordre actuel par défaut).
- `search_notes` : si `query.is_empty() && normalized_tags.is_empty()`,
  trier par `n.position ASC` (nouveau mode "ordre manuel"). Sinon,
  comportement actuel inchangé (pertinence FTS5 ou tags).
- Nouvelle fonction `db::move_note(conn, note_id, direction)` qui échange
  la `position` de la note avec sa voisine immédiate dans l'ordre manuel
  (haut ou bas), en transaction.
- UI : `gtk::DragSource` sur chaque `ListBoxRow` de la liste de notes (dans
  `library_panel`), `gtk::DropTarget` sur la `ListBox` elle-même, ne
  s'activant que lorsque le tri manuel est actif (pas de recherche/filtre).
  Pendant un drag actif sur une recherche/filtre en cours, le
  drag doit être désactivé (pas de sens de réordonner un sous-ensemble
  filtré) — le `DragSource` est simplement détaché/non connecté quand une
  recherche ou un filtre est actif, réévalué à chaque `refresh_notes`.
- **Équivalent clavier obligatoire (WCAG 2.1.1)** : sur la ligne
  sélectionnée de la liste de notes, `Alt+Flèche haut` /
  `Alt+Flèche bas` appellent `db::move_note` dans la direction
  correspondante, uniquement quand le tri manuel est actif. Ajout d'un
  `gtk::EventControllerKey` sur la `ListBox`.

## Fichiers impactés

- `migrations/001_init.sql` : inchangé (pas de migration versionnée
  formelle dans ce projet).
- `src/db.rs` : ajout migration procédurale colonne `position`, fonction
  `move_note`, modification de `search_notes` pour le tri conditionnel.
- `src/app.rs` : HeaderBar, fusion des boutons, textes français, labels
  visibles, bannière d'erreur, `text_scale` branché, aide raccourcis,
  drag & drop + raccourcis clavier de réordonnancement.
- `src/config.rs` : inchangé (le champ `text_scale` existe déjà).

## Hors périmètre

- Export Markdown, corbeille UI, chiffrement (roadmap V2+, non concernés
  ici).
- Système de migrations versionnées générique (on reste sur le pattern
  procédural existant, étendu au strict nécessaire).
