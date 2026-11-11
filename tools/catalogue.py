"""Liste des jeux du catalogue (partagée par extract_romsets.py et gen_tables.py)."""
# Jeux sans protection (cartouche MAME « rom »), clé de protection NONE.
PLAIN = ['2020bb', '3countb', 'alpham2', 'androdun', 'aodk', 'aof', 'aof2', 'aof3', 'b2b', 'bakatono', 'bjourney', 'blazstar', 'breakers', 'breakrev', 'bstars', 'bstars2', 'burningf', 'crswd2bl', 'crsword', 'ctomaday', 'cyberlip', 'diggerma', 'doubledr', 'eightman', 'fatfursp', 'fatfury1', 'fatfury3', 'fbfrenzy', 'fightfev', 'flipshot', 'froman2b', 'galaxyfg', 'ghostlop', 'goalx3', 'gowcaizr', 'gpilots', 'gururin', 'ironclad', 'irrmaze', 'janshin', 'joyjoy', 'kabukikl', 'karnovr', 'kizuna', 'kof94', 'kof95', 'kof96', 'kof97', 'kotm', 'kotm2', 'lastblad', 'lastbld2', 'lasthope', 'lbowling', 'legendos', 'lresort', 'magdrop2', 'magdrop3', 'maglord', 'mahretsu', 'marukodq', 'miexchng', 'minasan', 'moshougi', 'mutnat', 'nam1975', 'ncombat', 'ncommand', 'neobombe', 'neocup98', 'neodrift', 'neomrdo', 'ninjamas', 'overtop', 'panicbom', 'pbobbl2n', 'pbobblen', 'pgoal', 'popbounc', 'pspikes2', 'pulstar', 'puzzldpr', 'puzzledp', 'quizdai2', 'quizdais', 'quizkof', 'ragnagrd', 'rbff1', 'rbff2', 'rbffspec', 'ridhero', 'roboarmy', 'samsho', 'samsho2', 'samsho3', 'samsho4', 'savagere', 'sdodgeb', 'sengoku', 'sengoku2', 'shocktr2', 'shocktro', 'socbrawl', 'sonicwi2', 'sonicwi3', 'spinmast', 'ssideki2', 'ssideki3', 'ssideki4', 'stakwin', 'stakwin2', 'strhoop', 'superspy', 'tophuntr', 'tpgolf', 'trally', 'turfmast', 'twinspri', 'twsoc96', 'viewpoin', 'wakuwak7', 'wh1', 'wh2', 'wh2j', 'whp', 'wjammers', 'zedblade', 'zintrckb']
# Jeux déjà définis avec leur clé de protection (PROT_<CLE> dans types.rs).
PROTECTED = {
    "mslug": "NONE", "mslug2": "NONE", "mslugx": "MSX",
    "mslug3": "MS3", "mslug4": "MS4", "mslug5": "MS5",
    # T1005 : puce CMC42 seule (GFX + fixe chiffrés), pas de protection au runtime
    "zupapa": "C42_ZUPAPA", "ganryu": "C42_GANRYU", "s1945p": "C42_S1945P", "preisle2": "C42_PREISLE2",
    "bangbead": "C42_BANGBEAD", "nitd": "C42_NITD", "sengoku3": "C42_SENGOKU3",
    # T1006 : puce CMC50 (M1 chiffrée, C et S chiffrées)
    "kof2001": "C50_KOF2001", "jockeygp": "C50_JOCKEYGP",
    # T1007 : puce SMA
    "kof99": "SMA_KOF99", "garou": "SMA_GAROU", "kof2000": "SMA_KOF2000",
    # T1008 : CMC50 + brouillage PCM2 de la ROM V
    "rotd": "PCM2_ROTD", "pnyaa": "PCM2_PNYAA",
    # T1009 : famille K2K2 (P permutée par blocs, C50, PCM2 swap)
    "kof2002": "K2K2_KOF2002", "samsho5": "K2K2_SAMSHO5", "samsh5sp": "K2K2_SAMSH5SP",
    # T1010 : puce PVC (P déchiffrée, C50, PCM2 swap, port runtime PVC)
    "kof2003": "PVC_KOF2003", "svc": "PVC_SVC",
    # T1011 : kof98 (P chiffrée, recouvrement de ROM par la puce Altera)
    "kof98": "KOF98",
    # T1012 : puce PRO-CT0
    "fatfury2": "FATFURY2", "ssideki": "FATFURY2",
    # T1013 : sbp
    "sbp": "SBP",
}
# Jeux protégés extraits dès maintenant (ROM_START dans romsets_all.txt) mais définis par les cartes T1005+.
TO_EXTRACT = "kof98 fatfury2 ssideki sbp vliner kof99 garou kof2000 kof2001 jockeygp kof2002 kof2003 svc samsho5 samsh5sp rotd pnyaa zupapa ganryu s1945p preisle2 bangbead nitd sengoku3".split()
# Titres conservés tels quels (compatibilité avec les tests existants).
TITLES = {"mslug":"Metal Slug - Super Vehicle-001","mslug2":"Metal Slug 2 - Super Vehicle-001/II","mslugx":"Metal Slug X - Super Vehicle-001","mslug3":"Metal Slug 3","mslug4":"Metal Slug 4","mslug5":"Metal Slug 5"}
def catalogue():
    """[(nom, clé de protection)] dans l'ordre : Metal Slug puis PLAIN."""
    return list(PROTECTED.items()) + [(n, "NONE") for n in PLAIN]
