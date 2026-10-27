"""Traîne de la pièce : chute du niveau après la fin des mots (dB à 60, 120 et 200 ms), moyenne sur les mots suivis
d'un silence d'au moins 220 ms. Plus la chute est forte et rapide, plus la voix est sèche.
Usage : python traine.py <analyse.json> <debut_s> <wav>…
"""
import json, subprocess, sys
import numpy as np
SR = 48000
mots = json.load(open(sys.argv[1]))["mots"]; debut = float(sys.argv[2])
def lire(f):
    b = subprocess.run(["ffmpeg","-v","error","-i",f,"-ac","1","-ar",str(SR),"-f","f32le","-"],capture_output=True,check=True).stdout
    return np.frombuffer(b, np.float32)
def niveau(x, t, largeur=0.04):
    i = int(t*SR); seg = x[max(0,i-int(largeur*SR/2)):i+int(largeur*SR/2)]
    return 10*np.log10(np.mean(seg**2)+1e-12) if len(seg) else np.nan
for f in sys.argv[3:]:
    x = lire(f); duree = len(x)/SR
    fins = []
    for a, b in zip(mots, mots[1:]):
        t = a["t1"] - debut
        if 0.5 < t < duree - 0.5 and b["t0"] - a["t1"] >= 0.22:
            fins.append(t)
    chutes = {d: [] for d in (0.06, 0.12, 0.20)}
    for t in fins:
        ref = niveau(x, t - 0.06)
        for d in chutes:
            chutes[d].append(ref - niveau(x, t + d))
    print(f"{f.split('/')[-1]:<40} fins {len(fins):2d}  chute 60 ms {np.nanmean(chutes[0.06]):5.1f} dB  120 ms {np.nanmean(chutes[0.12]):5.1f} dB  200 ms {np.nanmean(chutes[0.20]):5.1f} dB")
