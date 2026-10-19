// Génère les 3 schémas de docs/coding-benchmarks.md.
//
// Régénérer, vérifier, exporter (depuis la racine du dépôt) :
//   node docs/coding-benchmarks/generate.mjs
//   node /opt/devcontainer/base/skills/diagram/scripts/check.mjs docs/coding-benchmarks
//   node ~/.cache/diagram-skill/scripts/export.mjs docs/coding-benchmarks
//
// L'export suppose la copie installée du skill /diagram (voir son SKILL.md).

import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { D } from '/opt/devcontainer/base/skills/diagram/scripts/exca.mjs';

const OUT = dirname(fileURLToPath(import.meta.url));
const BLUE = '#a5d8ff';
const YELLOW = '#ffec99';
const GREEN = '#b2f2bb';
const NOTE_X = 1160;

function save(name, d) {
  writeFileSync(join(OUT, `${name}.excalidraw`), `${JSON.stringify(d.toJSON(), null, 2)}\n`);
}

// SWE-bench Pro : un ticket, un patch, des tests cachés.
{
  const d = new D('SWE-bench Pro : corriger un ticket dans un vrai dépôt');
  d.node('repo', 260, 140, 'Dépôt au commit de base\n(41 dépôts, 1 865 tâches)', { bg: YELLOW });
  d.node('issue', 740, 140, 'Ticket réécrit par un humain\n(exigences + interfaces)', { bg: BLUE });
  d.node('agent', 500, 440, 'Agent = modèle + outil\nlit, édite, lance des tests', { bg: GREEN });
  d.node('patch', 500, 660, 'Patch (un git diff)', { bg: YELLOW });
  d.node('box', 500, 860, 'Conteneur vierge\n+ patch appliqué', { bg: GREEN });
  d.node('f2p', 260, 1160, 'Tests cachés fail-to-pass\nle bug est-il corrigé ?', { bg: BLUE });
  d.node('p2p', 740, 1160, 'Tests cachés pass-to-pass\nrien d\'autre n\'est cassé ?', { bg: BLUE });
  d.node('ok', 500, 1460, 'Réussi seulement si\nles DEUX passent');
  d.arrow('repo', 'agent');
  d.arrow('issue', 'agent');
  d.arrow('agent', 'patch');
  d.arrow('patch', 'box');
  d.arrow('box', 'f2p');
  d.arrow('box', 'p2p');
  d.arrow('f2p', 'ok');
  d.arrow('p2p', 'ok');
  d.float(NOTE_X, 140, [
    'Trois jeux de tâches :',
    '  public      731  (11 dépôts)',
    '  held-out    858  (12 dépôts)',
    '  commercial  276  (18 startups, code privé)',
    '',
    'Ce que ça mesure :',
    'corriger un bug ou ajouter une petite',
    'feature dans du code existant, à partir',
    'd\'un ticket clair. Le pain quotidien.',
    '',
    'Limite : le jeu public est en ligne,',
    'donc potentiellement mémorisé.',
  ].join('\n'));
  save('swe-bench-pro', d);
}

// DeepSWE : une feature inédite, longue, vérifiée sur le comportement.
{
  const d = new D('DeepSWE v1.1 : livrer une feature inédite, de bout en bout');
  d.node('repo', 260, 140, 'Dépôt open source actif\n(91 dépôts, 5 langages)', { bg: YELLOW });
  d.node('task', 740, 140, 'Tâche écrite de zéro,\njamais publiée', { bg: BLUE });
  d.node('agent', 500, 440, 'Agent, session longue :\nplanifier, coder sur plusieurs\nfichiers, tester, corriger', { bg: GREEN });
  d.node('state', 500, 700, 'État final du dépôt', { bg: YELLOW });
  d.node('verif', 500, 900, 'Vérificateur écrit à la main :\nteste le COMPORTEMENT observable,\npas l\'implémentation', { bg: GREEN });
  d.node('score', 500, 1160, 'Score = moyenne sur 4 essais\ncontexte saturé ou timeout\n= échec');
  d.arrow('repo', 'agent');
  d.arrow('task', 'agent');
  d.arrow('agent', 'state');
  d.arrow('state', 'verif');
  d.arrow('verif', 'score');
  d.float(NOTE_X, 140, [
    '113 tâches, 91 dépôts',
    'TypeScript, Go, Python, JavaScript, Rust',
    '',
    'Ce que ça mesure :',
    'implémenter une vraie feature sur la',
    'durée, sans solution connue à recopier.',
    'Le plus proche d\'une journée de dev.',
    '',
    'Atout : tâches inédites, donc',
    'impossibles à mémoriser.',
    'Plusieurs façons de coder sont',
    'acceptées tant que ça marche.',
  ].join('\n'));
  save('deepswe', d);
}

// Terminal-Bench 4.0 : se débrouiller seul dans un shell.
{
  const d = new D('Terminal-Bench 4.0 : accomplir une tâche dans un terminal');
  d.node('env', 260, 140, 'Conteneur Docker préparé\n(fichiers, outils, état cassé)', { bg: YELLOW });
  d.node('instr', 740, 140, 'Consigne en langage naturel', { bg: BLUE });
  d.node('agent', 500, 440, 'Agent dans un terminal', { bg: GREEN });
  d.node('shell', 960, 440, 'Shell (bash)', { bg: BLUE });
  d.node('state', 500, 700, 'État final du conteneur', { bg: YELLOW });
  d.node('tests', 500, 900, 'Script de test sur l\'état :\nça compile ? le service répond ?\nle fichier attendu existe ?', { bg: GREEN });
  d.node('score', 500, 1160, 'Réussi / échoué\n(jusqu\'à 8 h par tâche)');
  d.arrow('env', 'agent');
  d.arrow('instr', 'agent');
  d.arrow('agent', 'shell', { label: 'commande', shift: [0, -20] });
  d.arrow('shell', 'agent', { label: 'sortie', shift: [0, 20] });
  d.arrow('agent', 'state');
  d.arrow('state', 'tests');
  d.arrow('tests', 'score');
  d.float(NOTE_X, 640, [
    '66 tâches seulement (1 tâche = 1,5 point)',
    'Logiciel 18, Science 14, ML 11, Ops 9,',
    'Matériel 5, Sécurité 5, Média 4',
    '',
    'Ce que ça mesure :',
    'se débrouiller seul en ligne de commande :',
    'build, config, debug d\'environnement,',
    'scripts, outils. Moins du code pur.',
    '',
    'Limites : tâches publiques, et 30 sur 66',
    'ont un défaut de notation connu (Epoch).',
  ].join('\n'));
  save('terminal-bench', d);
}
