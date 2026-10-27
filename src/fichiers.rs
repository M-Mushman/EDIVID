//! Écritures de fichiers : toujours atomiques (fichier provisoire puis renommage), et réservées au propriétaire
//! (droits 600) quand elles portent un secret — code d'accès, jetons Google, mots de passe.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use anyhow::{Context, Result};

/// Écrit `octets` dans `chemin` d'un seul coup : un lecteur voit l'ancien fichier entier ou le nouveau, jamais
/// un fichier coupé. `secret` : droits 600 dès la création, avant qu'un octet soit écrit.
pub fn ecrire(chemin: &Path, octets: &[u8], secret: bool) -> Result<()> {
    let provisoire = chemin.with_extension(format!("{}.en-cours", chemin.extension().and_then(|e| e.to_str()).unwrap_or("")));
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(if secret { 0o600 } else { 0o644 })
            .open(&provisoire)
            .with_context(|| format!("écriture de {}", provisoire.display()))?;
        f.write_all(octets)?;
        f.sync_all()?;
    }
    std::fs::rename(&provisoire, chemin).with_context(|| format!("remplacement de {}", chemin.display()))?;
    Ok(())
}

/// Un identifiant venu de l'adresse (projet, rush, média, jeu…) ne désigne qu'un nom simple : jamais un chemin.
pub fn identifiant_sur(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiants() {
        assert!(identifiant_sur("9c8892ad3890"));
        assert!(identifiant_sur("jeu_1-a"));
        for mauvais in ["", "..", ".", "../..", "a/b", "a\\b", "%2E%2E", "a.b", "x\0"] {
            assert!(!identifiant_sur(mauvais), "{mauvais:?} accepté");
        }
    }

    #[test]
    fn ecriture_secrete_atomique() {
        let d = std::env::temp_dir().join(format!("edivid-essai-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("secret.json");
        ecrire(&f, b"{\"a\":1}", true).unwrap();
        ecrire(&f, b"{\"a\":2}", true).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "{\"a\":2}");
        assert!(!d.join("secret.json.en-cours").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
