# SonArcan — Cahier des charges

**Version :** 0.3 — choix technologique validé  
**Statut :** base de conception pour l’implémentation  
**Format de projet :** `.sac`  
**Technologie directrice :** Rust + Tauri 2 + TypeScript/Svelte  
**Slogan :** *Dive into the music.*

## 1. Objet du projet

SonArcan est une application desktop professionnelle destinée aux musiciens qui doivent apprendre, comprendre et reprendre rapidement les morceaux d’une playlist de groupe. Elle réunit dans une seule fenêtre la lecture audio, la visualisation, l’analyse musicale, la séparation de sources et l’édition des informations utiles au musicien.

Le parcours central est : **écouter → ralentir → boucler → isoler → analyser → comprendre → mémoriser → jouer**.

Le produit doit être conçu en priorité pour être :

- stable et prévisible ;
- exceptionnellement facile à diagnostiquer et à déboguer ;
- performant sur l’audio temps réel ;
- fluide pendant les traitements lourds et les traitements IA ;
- compatible Apple Silicon, puis Windows et Linux ;
- extensible sans réécriture de l’architecture centrale.

La V1 ne doit pas devenir un séquenceur ou un logiciel de production musicale complet. Les fonctions doivent rester centrées sur l’écoute, l’analyse, l’étude et l’organisation de morceaux.

## 2. Décisions prises

### 2.1 Choix technique

Le socle applicatif est Rust. Le framework UI validé est Tauri 2, avec une interface TypeScript/Svelte.

Tauri est retenu car il permet de conserver le cœur applicatif en Rust tout en offrant une interface moderne, légère et adaptée aux panneaux graphiques de SonArcan. Svelte est retenu pour limiter la complexité du frontend et faciliter le suivi de l’état de l’interface.

La phase 0 ne remet pas ce choix en concurrence : elle valide l’intégration, les performances de rendu et la qualité du diagnostic. Le moteur doit rester découplé de Tauri afin de permettre une évolution future de l’interface sans réécrire le cœur audio.

1. Tauri 2 + Svelte + TypeScript ;
2. Rust pour tout le cœur métier et temps réel ;
3. aucune technologie alternative n’est prévue pour la V1.

Le prototype doit être évalué sur des mesures réelles, et non sur une préférence théorique. Aucun changement de langage ou d’architecture ne doit être introduit sans justification documentée.

### 2.2 Priorités

Les priorités, dans l’ordre, sont :

1. stabilité et absence de corruption de projet ;
2. observabilité et qualité du debug ;
3. absence de perturbation de la lecture audio ;
4. performance et utilisation efficace de l’accélération matérielle ;
5. qualité et fluidité de l’interface ;
6. compatibilité multiplateforme.

## 3. Périmètre fonctionnel V1

### 3.1 Projets `.sac`

L’application doit permettre de créer, ouvrir, enregistrer, enregistrer sous et fermer un projet `.sac`.

Un projet doit conserver au minimum :

- les références ou copies des médias importés ;
- les métadonnées du morceau ;
- la playlist et l’ordre de lecture ;
- la position de lecture et les boucles ;
- les marqueurs et sections ;
- les réglages de tempo, BPM, tonalité et transposition ;
- les accords édités et leur mise en page ;
- l’état des analyses et des traitements IA ;
- la version du format et les informations de compatibilité.

Le format doit être versionné, validé à l’ouverture et migrable. Une sauvegarde interrompue ne doit pas détruire le dernier projet valide. Les écritures doivent être atomiques, avec récupération après interruption lorsque cela est possible.

Le projet doit pouvoir fonctionner avec des médias référencés à l’extérieur ou regroupés dans une archive/structure de projet, selon une règle explicite documentée par l’application.

### 3.2 Importation

L’utilisateur doit pouvoir importer des fichiers audio locaux, un dossier, une archive ZIP et des éléments par glisser-déposer. Les formats effectivement supportés doivent être déterminés par le backend de décodage et affichés clairement à l’utilisateur.

L’application doit également prévoir l’import de plusieurs URL YouTube par collage. Le téléchargement doit passer par un composant isolé de type `yt-dlp`, soumis à la disponibilité de l’outil, aux conditions du service et à une validation stricte des entrées. Les erreurs réseau, les contenus indisponibles et les formats non compatibles doivent être signalés sans bloquer l’application.

Les formats audio officiellement supportés en V1 sont **WAV, MP3 et FLAC**. Pour les téléchargements, le format par défaut est un **MP3 de bonne qualité**, suffisant pour le travail musical courant. L’utilisateur peut choisir une autre qualité ou demander une conversion lorsque cette option est disponible. Le format et la qualité finale doivent être visibles et sauvegardés dans le projet.

### 3.3 Playlist et lecture

La playlist doit permettre d’ajouter, supprimer, réordonner et sélectionner les morceaux importés. La lecture doit inclure lecture/pause, arrêt, navigation, recherche dans le morceau, volume et indication de l’état courant.

Elle doit proposer :

- une boucle A/B éditable ;
- la répétition du morceau ou de la playlist ;
- un affichage de la position et de la durée ;
- une lecture continue sans interruption visible lors du chargement d’analyses secondaires.

Le thread audio temps réel ne doit jamais effectuer d’accès disque bloquant, d’allocation imprévisible, d’appel réseau, d’inférence IA ou d’opération UI.

### 3.4 Waveform, spectre et timeline

L’interface doit afficher une waveform liée temporellement à l’audio, avec zoom et déplacement. Elle doit afficher la position de lecture, les points A/B et les marqueurs.

Un affichage du spectre doit être disponible avec une fréquence de rafraîchissement maîtrisée. Les calculs de waveform et de spectre doivent être pré-calculés ou exécutés dans des workers, avec cache réutilisable.

Toute modification de zoom, de fenêtre ou de traitement doit conserver la synchronisation entre audio, waveform, spectre, marqueurs et grille d’accords.

### 3.5 Tempo, métronome et édition temporelle

L’application doit permettre d’afficher et de modifier le BPM lorsqu’il est connu ou estimé. Elle doit prévoir un métronome synchronisé au tempo et une transposition de la vitesse de lecture.

Le time-stretch doit modifier la vitesse sans modifier la hauteur, dans les limites de qualité documentées. Le pitch-shifting doit modifier la hauteur sans modifier la vitesse, avec affichage du réglage et possibilité de retour à la valeur originale.

Les artefacts, limites et temps de calcul doivent être explicités dans les diagnostics lorsque le traitement ne peut pas être effectué en temps réel.

### 3.6 Marqueurs et sections

L’utilisateur doit pouvoir créer, modifier, déplacer et supprimer des marqueurs. Les marqueurs peuvent être nommés et associés à une couleur ou une catégorie.

L’application doit prévoir une détection automatique des sections du morceau. Cette détection est une aide : elle doit rester éditable et ne doit jamais écraser silencieusement les annotations manuelles.

### 3.7 Séparation audio et modèles IA

La séparation d’un morceau en stems doit être exécutée en arrière-plan. La V1 doit prévoir l’intégration d’un modèle de séparation de type RoFormer, ainsi que le chargement de modèles compatibles supplémentaires selon une interface de modèle documentée.

Les traitements IA doivent fournir :

- une progression ;
- un état en attente, en cours, terminé, annulé ou échoué ;
- l’accès aux logs et à l’erreur utile ;
- l’annulation lorsque techniquement possible ;
- la conservation des résultats valides en cache ;
- l’absence d’impact sur la lecture et l’interface.

Les modèles, poids, formats et licences ne doivent pas être incorporés implicitement au produit sans décision séparée. Un modèle défaillant ou incompatible doit être isolé et ne doit pas faire planter le processus principal.

L’accélération Apple Silicon doit être étudiée via les mécanismes adaptés à l’environnement Apple, notamment Core ML et/ou Metal. Les chemins GPU Windows/Linux devront être ajoutés seulement lorsqu’ils sont vérifiés et maintenables.

### 3.8 Analyse des accords

L’application doit pouvoir analyser un morceau afin de proposer des accords et leur position temporelle. Les résultats doivent être présentés comme des estimations et rester corrigibles manuellement.

La grille d’accords doit être éditable, navigable et imprimable. Un export PDF doit être prévu, avec une mise en page lisible et stable. Les résultats d’analyse doivent être versionnés ou recalculables afin d’éviter qu’une modification de modèle invalide silencieusement un projet existant.

## 4. Architecture cible

L’architecture doit séparer strictement les responsabilités :

```text
Interface
    ↓ événements / état observable
Application et cas d’usage
    ↓ commandes et événements typés
Projet / domaine musical
    ↓ services spécialisés
Audio engine — DSP — Analyse — IA — Workers
```

Les modules doivent être organisés pour limiter les dépendances circulaires et permettre des tests indépendants. Une organisation indicative est :

```text
crates/
  app/          orchestration et cas d’usage
  domain/       types projet, audio et analyse
  project/      format .sac, sauvegarde et migration
  audio/        moteur temps réel, décodage et sorties
  dsp/          time-stretch, pitch-shift, métronome
  analysis/     waveform, spectre, BPM, sections, accords
  inference/    modèles IA et accélération matérielle
  workers/      files de tâches, progression et annulation
  diagnostics/  logs, métriques, rapports et reproduction
  ui/           interface choisie après prototype
```

Cette organisation est une direction d’architecture, pas une obligation de multiplier les crates. Toute simplification doit préserver les frontières et les tests.

## 5. Robustesse et debug — exigences non négociables

Le produit doit intégrer dès le premier prototype :

- logs structurés avec niveaux configurables ;
- identifiant de session et identifiant de tâche ;
- journal des événements audio et des transitions d’état ;
- diagnostics de périphérique, latence, fréquence, buffer et underruns ;
- rapports de crash exploitables ;
- mode diagnostic exportable sans données inutiles ;
- métriques CPU, mémoire, temps de calcul et files d’attente ;
- outils de reproduction à partir d’un projet et d’une configuration ;
- messages d’erreur orientés action, avec cause et contexte ;
- séparation claire entre erreur récupérable, tâche échouée et panne fatale.

Les logs ne doivent pas introduire de blocage dans le thread audio ni exposer de données sensibles par défaut. Les traitements lourds doivent être traçables de leur lancement à leur résultat.

L’architecture doit permettre d’activer les assertions, sanitizers et outils de profiling dans les builds de développement. Les conditions de course, les accès invalides, les dépassements de budget audio et les fuites de ressources doivent être détectés par la CI ou par des outils dédiés.

## 6. Concurrence et mémoire

Le moteur audio doit avoir un contrat explicite de temps réel : aucune allocation non bornée, verrou bloquant, I/O ou appel imprévisible dans la callback audio.

Les workers doivent utiliser des files de tâches typées, des états observables et une annulation coopérative. Les résultats doivent être publiés de manière versionnée afin qu’un résultat ancien ne puisse écraser une modification plus récente.

Les caches doivent avoir une politique de taille, d’invalidation et de nettoyage. Les gros fichiers et stems ne doivent pas être chargés en mémoire sans limite documentée.

## 7. Interface et expérience utilisateur

L’application doit privilégier une seule fenêtre organisée autour de :

- une playlist ;
- une zone principale waveform/timeline ;
- un affichage spectre et informations musicales ;
- les commandes de lecture ;
- une zone de tâches et diagnostics ;
- les outils d’analyse, stems et accords.

L’interface doit rester réactive pendant l’import, le décodage, la génération de waveform, l’analyse et l’inférence. Toute tâche longue doit être visible, mais ne doit pas monopoliser l’écran ni empêcher la lecture si les ressources le permettent.

Les erreurs doivent être compréhensibles sans consulter les logs ; les logs détaillés doivent rester accessibles pour le diagnostic.

## 8. Sécurité et validation

Tous les chemins, archives, URL, modèles et métadonnées externes doivent être validés. L’extraction ZIP doit empêcher les chemins sortant du dossier cible. Les noms de fichiers, tailles, extensions et formats doivent être contrôlés.

Les commandes externes doivent être lancées sans concaténation shell non contrôlée. Les téléchargements doivent être limités, annulables et stockés dans un emplacement maîtrisé. Les données utilisateur ne doivent pas être envoyées à un service externe sans action explicite et information claire.

## 9. Tests et qualité

La CI doit couvrir au minimum :

- tests unitaires du domaine, du format `.sac` et des migrations ;
- tests de lecture/écriture atomique et récupération après interruption ;
- tests de décodage sur des fichiers représentatifs ;
- tests DSP avec tolérances documentées ;
- tests de synchronisation waveform/audio ;
- tests de workers, progression, annulation et erreurs ;
- tests d’import de fichiers, ZIP et lots d’URL ;
- tests de modèles absents, invalides et incompatibles ;
- tests de non-régression sur projets réels anonymisés ;
- tests de charge et de stabilité longue durée ;
- tests de démarrage et de fermeture propre.

Les tests audio doivent vérifier au minimum l’absence de silence inattendu, de discontinuité majeure, de désynchronisation et d’underrun dans les scénarios supportés.

## 10. Prototype technique obligatoire — phase 0

Avant le développement complet, construire un vertical slice minimal comprenant :

1. ouverture d’un MP3 et d’un FLAC ;
2. lecture et sortie audio ;
3. waveform et spectre ;
4. boucle A/B ;
5. time-stretch et pitch-shift ;
6. mesure de l’accélération Apple Silicon ;
7. séparation d’un morceau par un modèle ;
8. chargement d’un second modèle ou échec propre ;
9. tâche IA en arrière-plan avec progression ;
10. UI réactive pendant le traitement ;
11. logs structurés, diagnostics et rapport d’erreur ;
12. tests automatisés et profilage CPU/mémoire.

Le rapport de phase 0 doit valider la stack retenue sur : qualité audio, underruns, latence, temps de calcul, mémoire, fluidité UI, complexité de distribution, qualité des traces et facilité de reproduction des erreurs.

## 11. Phases de réalisation

### Phase 0 — validation technique

Prototype décrit ci-dessus, outillage de debug et premières mesures de référence.

### Phase 1 — fondations

Projet Rust, CI, diagnostics, format `.sac` minimal, import local, lecture, sauvegarde robuste et architecture des workers.

### Phase 2 — expérience audio

Playlist, waveform, spectre, timeline, navigation, boucle A/B, BPM, métronome, time-stretch et pitch-shift.

### Phase 3 — analyse et IA

Sections, marqueurs, séparation en stems, modèles supplémentaires, cache, accélération matérielle et analyse des accords.

### Phase 4 — édition et export

Grille d’accords éditable, impression, export PDF, import ZIP/URL, packaging et durcissement multiplateforme.

## 12. Critères d’acceptation V1

La V1 est acceptable lorsque :

- un projet `.sac` peut être créé, sauvegardé, rouvert et migré sans perte ;
- un fichier audio compatible peut être importé et lu sans blocage de l’interface ;
- waveform, spectre, timeline et audio restent synchronisés ;
- les boucles, marqueurs, BPM, tempo et transposition fonctionnent de manière vérifiable ;
- les analyses et traitements IA s’exécutent en arrière-plan avec progression et erreur explicite ;
- un échec de modèle, de fichier ou de téléchargement ne fait pas planter l’application ;
- les diagnostics permettent de reproduire et d’identifier les incidents importants ;
- les tests automatisés et les builds de distribution passent sur macOS Apple Silicon ;
- la compatibilité Windows/Linux est préparée et testée sur un périmètre défini ;
- la grille d’accords peut être corrigée et exportée en PDF ;
- aucune fonctionnalité V1 ne dépend d’un état implicite non sauvegardé.

Les seuils chiffrés de latence, CPU, mémoire, temps d’import et qualité de traitement devront être fixés à partir des résultats de la phase 0, avec une machine de référence documentée.

## 13. Hors périmètre initial

Sont hors périmètre jusqu’à décision spécifique :

- séquenceur multipiste complet ;
- enregistrement professionnel et mixage complet ;
- édition destructive avancée de formes d’onde ;
- collaboration temps réel ;
- service cloud obligatoire ;
- publication automatique vers des plateformes externes ;
- architecture hybride C++/Rust pour la V1 ;
- support de modèles ou de GPU non validés par le prototype.

## 14. Décisions arrêtées pour l’implémentation

Les choix suivants sont désormais la référence de travail et ne doivent pas être laissés en suspens pendant l’implémentation :

- **UI :** Tauri 2 + Svelte + TypeScript, avec composants dédiés à la waveform, au spectre et à la timeline. Le prototype de phase 0 valide cette intégration sans remettre en cause le moteur Rust.
- **Langage :** Rust stable pour le cœur, les commandes Tauri, le domaine, l’audio, les workers, l’import, les analyses et l’IA.
- **Formats V1 :** WAV, MP3 et FLAC. Aucun autre format ne doit être présenté comme officiellement supporté sans test d’intégration.
- **Téléchargement :** `yt-dlp` isolé dans un worker/processus contrôlé ; MP3 de bonne qualité par défaut ; choix de qualité et conversion optionnels lorsque disponibles ; téléchargement toujours asynchrone.
- **Projet :** package/dossier `.sac` inspectable, contenant des métadonnées ouvertes, des médias séparés, des résultats d’analyse et un cache supprimable. Les chemins relatifs sont privilégiés.
- **Plateformes :** macOS Apple Silicon en premier, Linux en deuxième, Windows en troisième. Les différences de backend audio/GPU doivent rester derrière des interfaces dédiées.
- **Crash reports :** génération locale de diagnostics et de rapports utiles dès la V1 ; collecte ou partage automatique reporté à une version ultérieure. Aucun fichier audio personnel ne doit être transmis automatiquement.
- **GPU :** accélération utilisée lorsqu’elle apporte un gain mesuré ; fallback CPU obligatoire et fonctionnel.
- **Modèles :** architecture multi-modèles, avec un premier modèle de séparation validé en phase 0, puis RoFormer/Demucs ou équivalent selon qualité, mémoire, licence et compatibilité Apple Silicon. L’installation de modèles personnels est acceptée uniquement lorsqu’ils respectent le contrat de modèle et peuvent être validés.
- **Limites produit :** SonArcan reste un outil d’immersion et de préparation musicale, pas un DAW, un séquenceur ou un logiciel de notation complet.

## 15. Réponses aux questions de conception

### 15.1 Stack technologique validée

| Couche | Technologie |
|---|---|
| Cœur applicatif, audio, DSP, IA, workers et projets | Rust stable |
| Conteneur desktop et communication IPC | Tauri 2 |
| Interface utilisateur | Svelte |
| Typage et contrats frontend | TypeScript |

Cette stack est désormais exclusive pour la V1. Le moteur Rust reste découplé de Tauri afin de préserver sa testabilité et sa maintenabilité.

### Le projet doit-il embarquer les médias ou les référencer ?

Par défaut, SonArcan conserve des références relatives vers un dossier de travail afin d’éviter de dupliquer de gros fichiers. L’utilisateur peut choisir un mode portable qui copie les médias dans le package `.sac`. Si un média manque, le projet reste ouvrable et propose une relocalisation.

### Le `.sac` doit-il être un fichier unique ou un dossier ?

Le `.sac` est un package/dossier. Cette structure facilite l’inspection, la récupération, les migrations et la distinction entre données importantes et caches régénérables. L’application devra toutefois offrir une ouverture simple depuis le Finder et les gestionnaires de fichiers.

### Quel format télécharger par défaut ?

MP3 de bonne qualité. C’est le meilleur compromis pour le travail musical courant, le stockage et la rapidité. WAV ou FLAC restent disponibles pour les utilisateurs qui privilégient la conservation sans perte ou une conversion spécifique.

### Faut-il partager les crash reports ?

Non pour la V1. L’application produit des rapports locaux exportables sur action de l’utilisateur. Une collecte distante pourra être ajoutée plus tard avec consentement explicite, anonymisation et contrôle des données.

### Quelle plateforme développer en premier ?

macOS Apple Silicon, car c’est la cible prioritaire pour l’accélération matérielle et la validation du moteur audio. Linux vient ensuite pour valider une architecture portable et ouverte, puis Windows avec ses backends audio et GPU spécifiques.

### Comment arbitrer performance et debug ?

Le debug et la stabilité priment sur un gain théorique. Toute optimisation doit être mesurée avant/après. Le callback audio reste minimal, les traitements sont isolés, les erreurs sont typées et chaque bug critique corrigé devient un test de régression.

### Comment garantir que Tauri reste déboguable ?

Les commandes Tauri doivent être de fines fonctions de validation et de traduction, jamais des emplacements de logique métier. Les échanges doivent utiliser des commandes typées pour les actions et des événements versionnés pour la progression et l’état. Les gros buffers audio ne doivent jamais transiter par JSON/IPC : le frontend reçoit uniquement des métadonnées, des tranches de visualisation ou des références de cache. Le moteur Rust possède ses propres logs, tests et diagnostics indépendamment de la webview. Aucun hybride C++/Rust ne sera introduit en V1.

## 16. Compléments fonctionnels intégrés

La V1 doit également couvrir les points suivants issus de la spécification complète :

- création d’un projet pour un groupe et choix d’un dossier de travail ;
- ouverture directe du package depuis le Finder ;
- import d’un fichier, de plusieurs fichiers, d’un dossier, d’un ZIP ou par glisser-déposer ;
- collage de plusieurs URL YouTube, détection des doublons et file de téléchargement ;
- import d’une vidéo ou d’une playlist YouTube lorsque `yt-dlp` le permet ;
- sélection et activation/désactivation des stems disponibles ;
- conservation non destructive de l’original, des réglages, analyses, accords et stems ;
- grille d’accords associée à des positions temporelles, modifiable et exportable en PDF ;
- raccourcis centralisés pour lecture, boucle, marqueurs, navigation et zoom ;
- jobs avec états `Queued`, `Running`, `Completed`, `Failed` et `Cancelled` ;
- diagnostic de l’OS, CPU, RAM, GPU, backend audio, fréquence, buffer, modèle et version de projet ;
- tests sur fichiers vides, corrompus, mono/stéréo, fréquences différentes, Unicode, gros fichiers, disque presque plein et fermeture pendant traitement.

## 17. Règles d’implémentation avec Codex

Codex doit développer SonArcan par tranches verticales vérifiables. Chaque étape doit fournir du code compilable, des tests, des logs, une documentation minimale et une fonctionnalité démontrable.

Avant toute modification importante, il doit comprendre le module concerné, ses dépendances et ses tests. Après modification, il doit compiler, exécuter les tests et signaler les régressions éventuelles.

L’ajout de bibliothèques et de packages est autorisé lorsqu’il est justifié et documenté. Dans ce cas uniquement, les audits réseau `npm audit` et OSV (`npm run security`) sont exécutés pour valider la nouvelle chaîne de dépendances. Ils ne sont pas relancés pour les changements ordinaires de code ou de documentation ; les modifications sensibles pour la sécurité font l’objet de validations ciblées sur la frontière de confiance concernée.

Pour chaque bug : reproduction, identification de la cause, correction minimale, test de régression et validation. Pour chaque optimisation : mesure, identification du goulot, modification, nouvelle mesure et contrôle de stabilité.

## 18. Résultat attendu

SonArcan doit permettre à un musicien de recevoir une playlist, l’écouter immédiatement, ralentir un passage, le boucler, isoler un instrument, visualiser sa structure, comprendre ses accords, corriger les résultats et retrouver l’ensemble de son travail dans un projet `.sac` fiable.

La règle de qualité absolue est : la stabilité audio, la réactivité, la sécurité mémoire, la fiabilité des projets et la capacité de diagnostic passent avant toute nouvelle fonctionnalité.
