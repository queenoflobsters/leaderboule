# il y a pleins de trucs surperflue parce que je voulais faire une simulation
from random import randint
import math
def creer_comptes(nb_joueur):
    l_joueur = []
    for i in range(nb_joueur):
        l_joueur.append([400, 40, 0, 0, 0, 0]) #elo, K, nb partie joué,victoire, défaite, diff moyenne (l'élo commence à 400 et le cas sert à dire si un joueur va gagner plus ou moins de point en une partie)
    return l_joueur
def calcul_elo (l_joueur, j_1, j_2):
    score = randint(0, 12)
    print(score)
    l_joueur[j_1][5] = ((l_joueur[j_1][2])*(l_joueur[j_1][5]) + (13-score)) /((l_joueur[j_1][2]) + 1 ) #ça permet de calcul la diff moyenne
    l_joueur[j_2][5] = ((l_joueur[j_2][2])*(l_joueur[j_2][5]) + (score - 13)) /(l_joueur[j_1][2] + 1)
    M = math.sqrt((13-score)/6.5) # le M permet de récompenser et punir les grands écart genre quand il y a 13-0
    prob = 1 / (1 + 10**(l_joueur[j_2][0]-l_joueur[j_1][0])) #la prob permet de faire qu'un hight élo ne gagne pas bcp contre un low élo et inversement
    print(f"c'est la prob {prob}")
    l_joueur[j_1][0] += int( M*l_joueur[j_1][1] * (1-prob))
    l_joueur[j_2][0] += int(M*l_joueur[j_1][1] * (0-prob))
    return l_joueur

def game(l_joueur):
    j_1 = int(randint(0,len(l_joueur)-1))
    j_2 = int(randint(0,len(l_joueur)-1))
    l_joueur[j_1][3] += 1
    l_joueur[j_2][4] += 1
    print(j_1)
    print(j_2)
    l_joueur[j_1][2] += 1  
    l_joueur[j_2][2] += 1
    if l_joueur[j_1][2] == 3 :#ça c'est pour le K en gros au début on change bcp d'élo c'est comme si on avait des games de placements puis on diminue si on joue plus de partie, on pourra peu être voir dans le futur à l'ajuster en fct du winrate genre quelqu'un qui a un gros winrate gagne plus à chaque partie que quelqu'un qui a un mauvais winrate 
        l_joueur[j_1][1] = 30
    elif l_joueur[j_1][2] == 20 :
        l_joueur[j_1][1] = 20
    if l_joueur[j_2][2] == 3 :
        l_joueur[j_2][1] = 30
    elif l_joueur[j_2][2] == 20 :
        l_joueur[j_2][1] = 20
    l_joueur = calcul_elo(l_joueur, j_1, j_2)
    return l_joueur

def simulation(nb_joueur, nb_game):
    l_joueur = creer_comptes(nb_joueur)
    for i in range (nb_game):
        l_joueur = game(l_joueur)
    return l_joueur

print(simulation(12, 100))


