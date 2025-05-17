# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

next = Suivant
unknown-os = OS inconnu
parttype-root = Racine du système de fichiers ({ $path })
parttype-esp = Partition EFI système ({ $path })
parttype-home = Données utilisateur ({ $path })
parttype-var = Données variables ({ $path })
page-welcome = Bienvenue dans { $distro }
page-welcome-try = Essayer
page-welcome-install = Installer
page-failure = Échec de l'installation
page-failure-close = Fermer
page-failure-bug = Signaler un bug
page-language = Langue
page-language-search-lang = Rechercher une langue/locale…
page-language-next = Suivant
page-completed = Terminé
page-completed-desc = L'installation est terminée. Vous pouvez maintenant redémarrer et profiter de votre nouveau système.
page-completed-close = Fermer
page-completed-reboot = Redémarrer
page-destination = Destination
page-destination-scanning = Scan des disques
page-destination-wait = En attente de os-prober…
page-destination-no-disk = Aucun disque trouvé
page-installdual = Double démarrage
page-installdual-otheros = Autre système d'exploitation
page-confirmation = Confirmation
page-installation = Installation
page-installation-welcome-desc = Apprenez à connaître votre nouveau système d'exploitation.
page-installation-help = Besoin d'aide ?
page-installation-help-desc = Posez vos questions dans l'un de nos chats !
page-installation-contrib = Contribuez à { $distro }
page-installation-progress = Installation du système de base...
page-installcustom = Installation personnalisée
page-installcustom-title = Partitions et points de montage
page-installcustom-desc = { $num } définition(s)
page-installcustom-tool = Ouvrir l'outil de partitionnement
page-installationtype = Type d'installation
page-installationtype-entire = Disque entier
page-installationtype-tpm = Activer le TPM
page-installationtype-chromebook = Chromebook
page-installationtype-custom = Personnalisée
dialog-installtype-encrypt = Chiffrement du disque
dialog-installtype-password = Mot de passe
dialog-installtype-repeat = Saisissez le mot de passe à nouveau
dialog-installtype-cancel = Annuler
dialog-installtype-confirm = Confirmer
installtype-edit-mp = Modifier le point de montage
installtype-rm-mp = Supprimer le point de montage
dialog-mp-part = Partition
dialog-mp-at = Monter sur
dialog-mp-opts = Options de montage
installtype-parttool = Sélectionnez votre outil de partitionnement
stage-extracting = Extraction des fichiers
stage-copying = Copie des fichiers
stage-initramfs = Régénération de l'initramfs
stage-grub1 = Génération du fichier grub.cfg d'étape 1 dans l'ESP...
stage-biosgrub = Installation de GRUB2 BIOS
stage-kernel = Réinstallation des noyaux
prev = Précédent
parttype-extendedboot = Partition étendue de bootloader ({ $path })
page-destination-no-disk-desc = Il n'y a aucun disque adapté à l'installation.
stage-mkpart = Création des partitions et copie des fichiers
page-welcome-desc = Vous pouvez essayer { $distro } ou commencer l'installation dès maintenant.
page-installation-contrib-desc = Découvrez comment contribuer en termes de temps, d'argent ou de matériel.
page-installcustom-add = Ajouter une nouvelle définition/ligne
parttype-other = Point de montage de partitionnement personnalisé
page-installationtype-encrypt = Activer le chiffrement du disque
page-installationtype-dual = Double démarrage
stage-grub2 = Génération du fichier grub.cfg d'étape 2 dans /boot/grub2/grub.cfg...
dialog-installtype-encrypt-desc =
    Veuillez définir le mot de passe de chiffrement du disque.
    Si vous perdez ce mot de passe, vos données ne seront pas récupérables.
stage-grub = Génération des valeurs système par défaut pour GRUB
stage-selinux = Définition des étiquettes SELinux
page-confirmation-problem-device-mounted = { $dev } est monté sur { $mountpoint }. Démontez-le pour continuer.
page-confirmation-problem-devblkopen =
    Le périphérique de blocs <tt>{ $dev }</tt> est utilisé par les processus suivants :
    <tt>{ $pids }</tt>
    Ces processus doivent être fermés avant que le programme d'installation ne puisse continuer.
